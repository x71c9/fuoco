use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "fuoco")]
#[command(about = "Deploy lightweight VMs on AWS, GCP, and Hetzner with a script", long_about = None)]
struct Cli {
  #[command(subcommand)]
  command: Commands,
}

#[derive(Subcommand)]
enum Commands {
  /// Deploy a VM
  Deploy {
    /// Cloud provider (aws, gcp, hetzner)
    #[arg(long)]
    cloud: String,

    /// Region to deploy the VM
    #[arg(long)]
    region: String,

    /// Path to the bash script to run on the VM
    #[arg(long)]
    script: String,
  },

  /// Destroy the VM
  Destroy {
    /// Cloud provider (aws, gcp, hetzner)
    #[arg(long)]
    cloud: String,

    /// Region of the VM to destroy
    #[arg(long)]
    region: String,
  },
}

fn main() {
  let cli = Cli::parse();

  match &cli.command {
    Commands::Deploy {
      cloud,
      region,
      script,
    } => {
      println!(
        "Deploying on {} in {} with script {}",
        cloud, region, script
      );
      // Placeholder for logic
    }
    Commands::Destroy { cloud, region } => {
      println!("Destroying on {} in {}", cloud, region);
      // Placeholder for logic
    }
  }
}
