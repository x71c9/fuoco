variable "region" {}
variable "instance_type" {}
variable "script_path" {}
variable "ssh_public_key_path" {
  type        = string
  description = "Path to SSH public key file"
  default     = null
}
variable "inbound_rules" {
  type = list(object({
    protocol     = string
    port_number  = number
  }))
  default = []
}

provider "aws" {
  region = var.region
}

locals {
  # List of fallback public key paths to auto-detect from
  fallback_key_paths = [
    pathexpand("~/.ssh/id_rsa.pub"),
    pathexpand("~/.ssh/id_ed25519.pub"),
    pathexpand("~/.ssh/id_ecdsa.pub")
  ]

  # Determine the first available fallback key path
  auto_detected_key_path = (
    length([
      for path in local.fallback_key_paths : path if fileexists(path)
    ]) > 0 ?
    [
      for path in local.fallback_key_paths : path if fileexists(path)
    ][0] :
    null
  )

  # Normalize the user-provided path: treat "none" or null as no input
  normalized_ssh_public_key_path = (
    var.ssh_public_key_path == null || var.ssh_public_key_path == "none"
    ? null
    : pathexpand(var.ssh_public_key_path)
  )

  # Final SSH public key path to use
  effective_ssh_public_key_path = (
    local.normalized_ssh_public_key_path != null
    ? local.normalized_ssh_public_key_path
    : local.auto_detected_key_path
  )
}

locals {
  # List of known ARM64-compatible instance types (add more if needed)
  arm64_instance_types = [
    "a1.medium", "a1.large", "a1.xlarge", "a1.2xlarge", "a1.4xlarge", "a1.metal",
    "t4g.nano", "t4g.micro", "t4g.small", "t4g.medium", "t4g.large", "t4g.xlarge", "t4g.2xlarge",
    "m6g.medium", "m6g.large", "m6g.xlarge", "m6g.2xlarge", "m6g.4xlarge", "m6g.8xlarge", "m6g.12xlarge", "m6g.16xlarge"
  ]

  # Infer architecture from instance type
  arch = contains(local.arm64_instance_types, var.instance_type) ? "arm64" : "x86_64"

  # Pick correct SSM parameter name for the AMI
  ami_ssm_param = local.arch == "arm64" ? "/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64" : "/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-x86_64"
}

# Resolve AMI from SSM Parameter Store
data "aws_ssm_parameter" "ami" {
  name = local.ami_ssm_param
}

# Use default VPC
data "aws_vpc" "default" {
  default = true
}

resource "aws_key_pair" "deployer" {
  count      = local.effective_ssh_public_key_path != null ? 1 : 0
  key_name   = "fuoco-ephemeral-key"
  public_key = file(local.effective_ssh_public_key_path)
}

# Security group allowing all traffic (for development/testing)
resource "aws_security_group" "allow_all" {
  name        = "fuoco-ephemeral-sg"
  description = "Allow all inbound and outbound traffic"
  vpc_id      = data.aws_vpc.default.id

  dynamic "ingress" {
    for_each = var.inbound_rules
    content {
      protocol    = ingress.value.protocol
      from_port   = ingress.value.port_number
      to_port     = ingress.value.port_number
      cidr_blocks = ["0.0.0.0/0"]
    }
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = {
    Name = "fuoco-ephemeral-sg"
  }
}

# EC2 Instance
resource "aws_instance" "vm" {
  ami                         = data.aws_ssm_parameter.ami.value
  instance_type               = var.instance_type
  user_data                   = var.script_path != "" ? var.script_path : null
  vpc_security_group_ids      = [aws_security_group.allow_all.id]
  key_name                    = aws_key_pair.deployer[0].key_name

  tags = {
    Name = "fuoco-ephemeral"
  }
}

# Outputs
output "public_ip" {
  value = aws_instance.vm.public_ip
}

output "region" {
  value = var.region
}

output "ssh_key_used" {
  value       = local.effective_ssh_public_key_path
  description = "Path to the SSH public key used for the instance"
}

output "inbound_rules" {
  value       = var.inbound_rules
  description = "List of inbound rules applied to the security group"
}
