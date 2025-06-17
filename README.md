# fuoco

Ephemeral VM provisioner for AWS, GCP, and Hetzner.

`fuoco` automates a built-in Terraform template to provision a single VM in AWS, GCP, or Hetzner,
executes a startup script via cloud-init/user-data, and destroys all resources on termination.

## Features

- **Single-VM workflow** – Apply and destroy with a single command.
- **Multi-cloud support** – AWS | GCP | Hetzner through a unified CLI.
- **Built-in Terraform templates** – No separate Terraform code to maintain.
- **Startup script support** – Inject Bash scripts at boot via cloud-init.
- **Debug mode** – `--debug` streams Terraform logs for troubleshooting.
- **Automatic cleanup** – Ensures a fresh workspace on every run.

## Requirements

- Rust (edition 2024) toolchain to build `fuoco`.
- Terraform (>= 0.12) available in system `PATH`.
- Cloud credentials:
  - **AWS**: via `~/.aws/credentials` or environment variables.
  - **GCP**: via `gcloud auth application-default login` or `GOOGLE_CLOUD_PROJECT` env var.
  - **Hetzner**: via `--hetzner-token` flag or `HCLOUD_TOKEN` env var.


## Usage

```bash
fuoco deploy --cloud <aws|gcp|hetzner> [OPTIONS]
```

| Option                       | Description                                                                                  |
|------------------------------|----------------------------------------------------------------------------------------------|
| `--cloud <aws|gcp|hetzner>`  | Cloud to deploy (aws, gcp, or hetzner).                                                      |
| `--region <REGION>`          | AWS region, GCP zone, or Hetzner location (e.g. `us-east-1`, `us-central1-a`, `nbg1`).       |
| `--instance-type <TYPE>`     | VM size (defaults: `t4g.nano` AWS, `e2-micro` GCP, `cx11` Hetzner).                          |
| `--project <PROJECT>`        | GCP project ID (or set `GOOGLE_CLOUD_PROJECT`).                                              |
| `--hetzner-token <TOKEN>`    | Hetzner API token (or set `HCLOUD_TOKEN`).                                                   |
| `--script <FILE>`            | Path to a Bash script to execute on VM startup.                                              |
| `--debug`                    | Print Terraform init/apply/destroy logs (for debugging).                                     |
| `-h, --help`                 | Show this help message.                                                                      |

Press <kbd>Ctrl+C</kbd> or send `SIGTERM` to destroy the VM and exit.

### Examples

```bash
# AWS example (default t4g.nano)
fuoco deploy --cloud aws --region us-east-1 --script ./startup.sh --debug

# GCP example (default e2-micro)
GOOGLE_CLOUD_PROJECT=my-project \
fuoco deploy --cloud gcp --region us-central1-a --script ./startup.sh --debug

# Hetzner example (default cx11)
HCLOUD_TOKEN=$HCLOUD_TOKEN \
fuoco deploy --cloud hetzner --region nbg1 --script ./startup.sh --debug
```

## Built‑in Terraform Templates

Templates embedded under `terraform/<provider>/main.tf`:

- **AWS**: Amazon Linux 2023 ARM64 via SSM `/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64`
- **GCP**: Ubuntu 20.04 LTS image family (`ubuntu-2004-lts`)
- **Hetzner**: Ubuntu 22.04 image alias

Each template also creates a security group that opens SSH/ICMP from anywhere by default.

## How It Works

1. Prepare a hashed temp workspace and copy the Terraform files.
2. `terraform init` the provider.
3. `terraform apply` with auto-approve and injected vars (region, instance-type, script, etc.).
4. Wait for `Ctrl+C`/`SIGTERM` (or panic) to trigger `terraform destroy`.
5. On each run, remove stale workspace so you always use the latest templates.

## Debugging & Troubleshooting
- Use `--debug` to view full Terraform logs.
- AWS Session Manager (SSM) can be enabled to access instance logs via the Systems Manager console.
- For console‑level logs, add `tee /dev/console` to the cloud‑init user‑data sequence.

## Contributing

Contributions welcome! Open issues or PRs for bugs, features, or improvements.
