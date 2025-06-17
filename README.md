# fuoco

Ephemeral VM provisioner for AWS, GCP, and Hetzner.

## Requirements

- Terraform (>= 0.12) installed and in your `PATH`.
- AWS credentials configured (e.g. via `~/.aws/credentials` or environment variables).
- GCP credentials configured (e.g. via `gcloud auth application-default login`).
- Hetzner API token provided via `--hetzner-token` flag or `HCLOUD_TOKEN` env var.
*Optionally* you can set `GOOGLE_CLOUD_PROJECT` to avoid passing `--project`.
- Pass `--debug` to display Terraform init/apply/destroy logs.

## Default Images

- **AWS**: Amazon Linux 2023 ARM64 via SSM parameter `/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64`
- **GCP**: Ubuntu 20.04 LTS image family (`ubuntu-2004-lts`)
- **Hetzner**: Ubuntu 22.04 image alias

## Default Sizes

- **AWS**: `t4g.nano` (cheapest ARM instance)
- **GCP**: `e2-micro`
- **Hetzner**: `cx11`

## Usage

# Add `--debug` to show Terraform init/apply/destroy logs.
```bash
# AWS example (uses default t4g.nano instance type)
fuoco deploy --cloud aws --region us-east-1 --script ./my-bash-script.sh

# GCP example (uses default e2-micro machine type)
fuoco deploy --cloud gcp --project my-project --region us-central1-a --script ./my-bash-script.sh

# Hetzner example (uses default cx11 server type)
fuoco deploy --cloud hetzner --hetzner-token $HCLOUD_TOKEN --region nbg1 --script ./my-bash-script.sh
```

After successful deployment, press <kbd>Ctrl+C</kbd> (or send SIGTERM) to destroy resources and exit.
