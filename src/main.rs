use anyhow::{Context, Result};
use atar::{deploy as lib_deploy, undeploy as lib_undeploy};
use clap::{Parser, Subcommand, ValueEnum};
use sha2::{Digest, Sha256};
use signal_hook::{
  consts::signal::{SIGINT, SIGTERM},
  iterator::Signals,
};
use std::{
  collections::HashMap, env, fs, panic, path::PathBuf, process, sync::mpsc,
  thread,
};

/// fuoco: Ephemeral VM deployer that applies a Terraform template,
/// optionally runs a startup script via cloud-init, and destroys on exit.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
  #[command(subcommand)]
  command: Commands,
}

#[derive(Subcommand)]
enum Commands {
  /// Deploy an ephemeral VM and optionally run a startup script.
  Deploy {
    /// Cloud provider to deploy to (aws, gcp, hetzner).
    #[arg(long, value_enum)]
    cloud: Cloud,
    /// Enable debug mode (show Terraform stdout/stderr).
    #[arg(long)]
    debug: bool,
    /// Cloud region (AWS region, GCP zone, or Hetzner location).
    #[arg(long)]
    region: Option<String>,
    /// Instance type (default: t4g.nano for AWS, e2-micro for GCP, cx11 for Hetzner).
    #[arg(long, default_value = "t4g.nano")]
    instance_type: String,
    /// GCP project ID (or set GOOGLE_CLOUD_PROJECT environment variable).
    #[arg(long)]
    project: Option<String>,
    /// Hetzner API token (or set HCLOUD_TOKEN environment variable).
    #[arg(long = "hetzner-token")]
    hetzner_token: Option<String>,
    /// Path to a local script that will be executed on VM startup.
    #[arg(long)]
    script: Option<PathBuf>,
  },
}

/// Supported cloud providers.
#[derive(ValueEnum, Clone)]
enum Cloud {
  Aws,
  Gcp,
  Hetzner,
}

fn main() {
  if let Err(err) = run() {
    eprintln!("Error: {}", err);
    process::exit(1);
  }
}

fn run() -> Result<()> {
  let cli = Cli::parse();

  let (tf_path, vars, debug) = match cli.command {
    Commands::Deploy {
      cloud,
      debug,
      region,
      instance_type,
      project,
      hetzner_token,
      script,
    } => {
      let mut v = HashMap::new();
      let provider = match cloud {
        Cloud::Aws => {
          let r = region.context("--region is required for AWS")?;
          v.insert("region".into(), r);
          v.insert("instance_type".into(), instance_type);
          "aws"
        }
        Cloud::Gcp => {
          let z = region.context("--region (zone) is required for GCP")?;
          let p = project
            .or_else(|| env::var("GOOGLE_CLOUD_PROJECT").ok())
            .context(
              "--project or GOOGLE_CLOUD_PROJECT env var is required for GCP",
            )?;
          v.insert("project".into(), p);
          v.insert("zone".into(), z);
          v.insert("instance_type".into(), instance_type);
          "gcp"
        }
        Cloud::Hetzner => {
          let loc =
            region.context("--region (location) is required for Hetzner")?;
          let t = hetzner_token
            .or_else(|| env::var("HCLOUD_TOKEN").ok())
            .context(
              "--hetzner-token or HCLOUD_TOKEN env var is required for Hetzner",
            )?;
          v.insert("token".into(), t);
          v.insert("location".into(), loc);
          v.insert("instance_type".into(), instance_type);
          "hetzner"
        }
      };
      if let Some(path) = script {
        let content =
          fs::read_to_string(&path).context("Failed to read script file")?;
        v.insert("script".into(), content);
      }
      let path = template_path(provider)?;
      (path, v, debug)
    }
  };

  run_deploy(tf_path, vars, debug)?;
  Ok(())
}

/// Determine the path to the Terraform template for the given provider.
fn template_path(provider: &str) -> Result<PathBuf> {
  let manifest =
    env::var("CARGO_MANIFEST_DIR").context("CARGO_MANIFEST_DIR is not set")?;
  let mut path = PathBuf::from(manifest);
  path.push("terraform");
  path.push(provider);
  path.push("main.tf");
  Ok(path)
}

fn run_deploy(
  file: PathBuf,
  vars: HashMap<String, String>,
  debug: bool,
) -> Result<()> {
  println!("Variables:");
  println!("  path: {}", file.display());
  for (k, v) in &vars {
    println!("  {}: {}", k, v);
  }

  // Remove any existing cached Terraform workspace so changes to templates are picked up
  {
    let src_dir = file
      .parent()
      .context("Cannot determine Terraform directory")?;
    let mut hasher = Sha256::new();
    hasher.update(src_dir.to_string_lossy().as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    let work = env::temp_dir().join("atar").join(hash);
    if work.exists() {
      fs::remove_dir_all(&work)
        .context("Failed to remove stale Terraform workspace")?;
    }
  }
  let outputs = lib_deploy(&file, &vars, debug)?;
  if !outputs.is_empty() {
    println!("*************************** Outputs **************************");
    for (k, v) in outputs {
      println!("{}: {}", k, v);
    }
    println!("**************************************************************");
  }

  let guard = DestroyGuard {
    file: file.clone(),
    vars: vars.clone(),
    debug,
  };
  {
    let fh = file.clone();
    let vh = vars.clone();
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
      eprintln!("panic: {:?}, cleaning up Terraform...", info);
      if let Err(err) = lib_undeploy(&fh, &vh, debug) {
        eprintln!("cleanup after panic failed: {}", err);
      }
      previous(info);
    }));
  }

  let (tx, rx) = mpsc::channel();
  let mut signals =
    Signals::new(&[SIGINT, SIGTERM]).context("Failed to set signal handler")?;
  thread::spawn(move || {
    for _ in signals.forever() {
      let _ = tx.send(());
      break;
    }
  });
  println!(
    "Resources deployed.\n\nPress Ctrl+C or send SIGTERM to destroy and exit."
  );
  let _ = rx.recv();
  println!("\nSignal received: starting Terraform destroy...");
  drop(guard);
  Ok(())
}

struct DestroyGuard {
  file: PathBuf,
  vars: HashMap<String, String>,
  debug: bool,
}

impl Drop for DestroyGuard {
  fn drop(&mut self) {
    if let Err(err) = lib_undeploy(&self.file, &self.vars, self.debug) {
      eprintln!("Failed to destroy Terraform resources: {}", err);
    }
  }
}
