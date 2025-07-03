use anyhow::{Context, Result};
use atar::{deploy as lib_deploy, undeploy as lib_undeploy};
use clap::{Parser, Subcommand, ValueEnum};
use rand::seq::SliceRandom;
use serde::Serialize;
use serde_json;
use sha2::{Digest, Sha256};
use signal_hook::{
  consts::signal::{SIGINT, SIGTERM},
  iterator::Signals,
};
use std::fmt;
use std::{
  collections::HashMap, env, fs, panic, path::PathBuf, process, sync::mpsc,
  thread,
};

/// fuoco: Ephemeral VM deployer that applies a Terraform template,
/// and runs a startup script via cloud-init, then it destroys on exit.
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
    /// Enable debug mode (show Terraform stdout/stderr).
    #[arg(long, short = 'd')]
    debug: bool,
    /// Instance type (default: t4g.nano for AWS, e2-micro for GCP, cx11 for Hetzner).
    #[arg(long, short = 'i')]
    instance_type: Option<String>,
    /// Cloud provider to deploy to (aws, gcp, hetzner).
    #[arg(long, value_enum, short = 'c')]
    provider: Provider,
    /// Cloud region (AWS region, GCP zone, or Hetzner location).
    #[arg(long, short = 'r')]
    region: Option<String>,
    /// Path to a Bash script to execute on VM startup.
    #[arg(long, short = 's')]
    script_path: Option<PathBuf>,
    /// Inbound rules in the format protocol:port (e.g., tcp:22).
    #[arg(
      long = "inbound-rule",
      value_parser,
      value_name = "PROTO:PORT",
      short = 'p'
    )]
    inbound_rules: Option<Vec<InboundRule>>,
    /// Path to the public key that must be uploaded to the machine
    #[arg(long = "ssh-public-key-path", short = 'k')]
    ssh_public_key_path: Option<String>,
  },
  /// Destroy an existing ephemeral VM deployment.
  Undeploy {
    /// Enable debug mode (show Terraform stdout/stderr).
    #[arg(long, short = 'd')]
    debug: bool,
    /// Instance type (default: t4g.nano for AWS, e2-micro for GCP, cx11 for Hetzner).
    #[arg(long, short = 'i')]
    instance_type: Option<String>,
    /// Cloud provider to undeploy (aws, gcp, hetzner).
    #[arg(long, value_enum, short = 'c')]
    provider: Provider,
    /// Cloud region (AWS region, GCP zone, or Hetzner location).
    #[arg(long, short = 'r')]
    region: String,
  },
}

#[derive(Clone)]
struct RunDeployParams {
  debug: bool,
  instance_type: Option<String>,
  provider: Provider,
  region: Option<String>,
  script_path: Option<PathBuf>,
  template_path: PathBuf,
  inbound_rules: Option<Vec<InboundRule>>,
  ssh_public_key_path: Option<String>,
}

struct RunUndeployParams {
  debug: bool,
  instance_type: Option<String>,
  provider: Provider,
  region: String,
  template_path: PathBuf,
}

impl fmt::Debug for RunDeployParams {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    // Manually printing fields as key-value pairs
    write!(f, "Deploy params \n")?;
    write!(f, "  debug: {},\n", self.debug)?;
    let default_instance_type = resolve_default_instance_type(&self.provider);
    write!(
      f,
      "  instance_type: {},\n",
      self
        .instance_type
        .as_ref()
        .map_or(default_instance_type, |s| format!("[{}]", s))
    )?;
    write!(f, "  provider: {:?},\n", self.provider)?;
    write!(
      f,
      "  region: {},\n",
      self.region.as_ref().map_or("[Random]", |s| s)
    )?;
    let defulat_inbound_rules = resolve_default_inbound_rule();
    write!(f, "  script_path: {:?},\n", self.script_path)?;
    write!(f, "  template_path: {:?}\n", self.template_path)?;
    write!(
      f,
      "  inbound_rules: {:?}\n",
      self
        .inbound_rules
        .as_ref()
        .map_or(defulat_inbound_rules, |s| s.clone())
    )?;
    write!(
      f,
      "  ssh_public_key_path: {:?}\n",
      self.ssh_public_key_path.as_ref().map_or("[Default]", |s| s)
    )?;
    write!(f, "")
  }
}

impl RunDeployParams {
  fn to_atar_map(&self) -> HashMap<String, String> {
    let mut map = HashMap::new();
    // Convert each field to a String and insert it into the map
    let default_instance_type = resolve_default_instance_type(&self.provider);
    map.insert(
      "instance_type".to_string(),
      self
        .instance_type
        .as_ref()
        .map_or(default_instance_type, |s| s.clone()),
    );
    let random_region = resolve_random_region(&self.provider);
    map.insert(
      "region".to_string(),
      self.region.as_ref().map_or(random_region, |s| s.clone()),
    );
    let default_script_path = String::new();
    map.insert(
      "script_path".to_string(),
      self
        .script_path
        .as_ref()
        .map_or(default_script_path, |s| s.to_string_lossy().to_string()),
    );
    let defulat_inbound_rules = resolve_default_inbound_rule();
    let final_inbound_rules = &self
      .inbound_rules
      .as_ref()
      .map_or(defulat_inbound_rules, |s| s.clone());
    let inbound_rules_json =
      serde_json::to_string(final_inbound_rules).unwrap();
    map.insert("inbound_rules".to_string(), inbound_rules_json);
    let default_ssh_public_key_path = "none".to_string();
    map.insert(
      "ssh_public_key_path".to_string(),
      self
        .ssh_public_key_path
        .as_ref()
        .map_or(default_ssh_public_key_path, |s| s.clone()),
    );
    map
  }
}

impl fmt::Debug for RunUndeployParams {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    // Manually printing fields as key-value pairs
    write!(f, "Undeploy params \n")?;
    write!(f, "  debug: {},\n", self.debug)?;
    let default_instance_type = resolve_default_instance_type(&self.provider);
    write!(
      f,
      "  instance_type: {},\n",
      self
        .instance_type
        .as_ref()
        .map_or(default_instance_type, |s| format!("[{}]", s))
    )?;
    write!(f, "  provider: {:?},\n", self.provider)?;
    write!(f, "  region: {},\n", self.region)?;
    write!(f, "  template_path: {:?}\n", self.template_path)?;
    write!(f, "")
  }
}

impl RunUndeployParams {
  fn to_atar_map(&self) -> HashMap<String, String> {
    let mut map = HashMap::new();
    // Convert each field to a String and insert it into the map
    let default_instance_type = resolve_default_instance_type(&self.provider);
    map.insert(
      "instance_type".to_string(),
      self
        .instance_type
        .as_ref()
        .map_or(default_instance_type, |s| s.clone()),
    );
    map.insert("region".to_string(), self.region.clone());
    map
  }
}

/// Supported cloud providers.
#[derive(ValueEnum, Clone, Debug)]
enum Provider {
  AWS,
  GCP,
  Hetzner,
}

#[derive(Clone, Debug, Serialize)]
struct InboundRule {
  protocol: String,
  port_number: u16,
}

impl std::str::FromStr for InboundRule {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
      return Err("Inbound rule must be in format protocol:port".into());
    }
    let protocol = parts[0].to_string();
    let port_number = parts[1]
      .parse::<u16>()
      .map_err(|_| "Invalid port number".to_string())?;
    Ok(InboundRule {
      protocol,
      port_number,
    })
  }
}

fn main() {
  run().unwrap_or_else(|err| {
    eprintln!("Error: {}", err);
    process::exit(1);
  });
}

fn run() -> Result<()> {
  let cli = Cli::parse();

  match cli.command {
    Commands::Deploy {
      debug,
      instance_type,
      provider,
      region,
      script_path,
      inbound_rules,
      ssh_public_key_path,
    } => {
      let provider_str = match provider {
        Provider::AWS => "aws",
        Provider::GCP => "gcp",
        Provider::Hetzner => "hetzner",
      };
      let template_path = template_path(provider_str)?;
      let run_deploy_params = RunDeployParams {
        debug,
        instance_type,
        provider,
        region,
        script_path,
        template_path,
        inbound_rules,
        ssh_public_key_path,
      };
      run_deploy(run_deploy_params)?;
    }
    Commands::Undeploy {
      debug,
      instance_type,
      provider,
      region,
    } => {
      let provider_str = match provider {
        Provider::AWS => "aws",
        Provider::GCP => "gcp",
        Provider::Hetzner => "hetzner",
      };
      let template_path = template_path(provider_str)?;
      let run_undeploy_params = RunUndeployParams {
        debug,
        instance_type,
        provider,
        region,
        template_path,
      };
      run_undeploy(run_undeploy_params)?;
    }
  }
  Ok(())
}

/// Determine the path to the Terraform template for the given provider.
fn template_path(provider_str: &str) -> Result<PathBuf> {
  let manifest =
    env::var("CARGO_MANIFEST_DIR").context("CARGO_MANIFEST_DIR is not set")?;
  let mut path = PathBuf::from(manifest);
  path.push("templates");
  path.push(provider_str);
  path.push("main.tf");
  Ok(path)
}

fn run_deploy(params: RunDeployParams) -> Result<()> {
  println!("{:?}", params);
  // Remove any existing cached Terraform workspace so changes to templates are picked up
  {
    let template_dir = params
      .template_path
      .parent()
      .context("Cannot determine Terraform directory")?;
    let mut hasher = Sha256::new();
    hasher.update(template_dir.to_string_lossy().as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    let work = env::temp_dir().join("atar").join(hash);
    if work.exists() {
      fs::remove_dir_all(&work)
        .context("Failed to remove stale Terraform workspace")?;
    }
  }
  let hash_map = params.to_atar_map();
  let outputs = lib_deploy(&params.template_path, &hash_map, params.debug)?;
  if !outputs.is_empty() {
    println!("*************************** Outputs **************************");
    for (k, v) in outputs {
      println!("{}: {}", k, v);
    }
    println!("**************************************************************");
  }

  let guard = DestroyGuard {
    params: params.clone(),
  };
  {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
      eprintln!("panic: {:?}, cleaning up Terraform...", info);
      if let Err(err) =
        lib_undeploy(&params.template_path, &hash_map, params.debug)
      {
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

fn run_undeploy(params: RunUndeployParams) -> Result<()> {
  println!("{:?}", params);
  let hash_map = params.to_atar_map();
  lib_undeploy(&params.template_path, &hash_map, params.debug)?;
  Ok(())
}

struct DestroyGuard {
  params: RunDeployParams,
}

impl Drop for DestroyGuard {
  fn drop(&mut self) {
    let hash_map = self.params.to_atar_map();
    lib_undeploy(&self.params.template_path, &hash_map, self.params.debug)
      .unwrap_or_else(|err| {
        eprintln!("Failed to destroy Terraform resources: {}", err);
      });
  }
}
fn resolve_random_region(provider: &Provider) -> String {
  let aws_regions = vec![
    "us-east-1",
    "us-east-2",
    "us-west-1",
    "us-west-2",
    "ap-south-1",
    "ap-northeast-3",
    "ap-northeast-2",
    "ap-southeast-1",
    "ap-southeast-2",
    "ap-northeast-1",
    "ca-central-1",
    "eu-central-1",
    "eu-west-1",
    "eu-west-2",
    "eu-west-3",
    "eu-north-1",
    "sa-east-1",
  ];
  let gcp_regions = vec![
    "us-central1",
    "us-east1",
    "us-east4",
    "us-west1",
    "us-west2",
    "us-west3",
    "us-west4",
    "northamerica-northeast1",
    "southamerica-east1",
    "europe-west1",
    "europe-west2",
    "europe-west3",
    "europe-west4",
    "europe-west6",
    "europe-west8",
    "europe-west9",
    "europe-north1",
    "europe-southwest1",
    "asia-east1",
    "asia-east2",
    "asia-northeast1",
    "asia-northeast2",
    "asia-northeast3",
    "asia-south1",
    "asia-south2",
    "asia-southeast1",
    "asia-southeast2",
    "australia-southeast1",
    "australia-southeast2",
    "me-central1",
    "me-west1",
  ];
  let hetzner_regions = vec!["fsn1", "nbg1", "hel1", "ash", "hil"];
  match provider {
    Provider::AWS => aws_regions
      .choose(&mut rand::thread_rng())
      .expect("Cannot resolve random region for AWS")
      .to_string(),
    Provider::GCP => gcp_regions
      .choose(&mut rand::thread_rng())
      .expect("Cannot resolve random region for GCP")
      .to_string(),
    Provider::Hetzner => hetzner_regions
      .choose(&mut rand::thread_rng())
      .expect("Cannot resolve random region for Hetzner")
      .to_string(),
  }
}

fn resolve_default_inbound_rule() -> Vec<InboundRule> {
  return vec![InboundRule {
    protocol: "tcp".to_string(),
    port_number: 22,
  }];
}

fn resolve_default_instance_type(provider: &Provider) -> String {
  match provider {
    Provider::AWS => "t3.micro".to_string(),
    Provider::GCP => "f1-micro".to_string(),
    Provider::Hetzner => "cx11".to_string(),
  }
}
