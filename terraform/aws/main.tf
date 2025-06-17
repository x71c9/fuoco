variable "region" {}
variable "instance_type" {}
variable "script" { default = "" }

provider "aws" {
  region = var.region
}

// Fetch latest Amazon Linux 2023 AMI for ARM64 (region-agnostic) via SSM Parameter Store
data "aws_ssm_parameter" "ami" {
  name = "/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64"
}

// Use the default VPC
data "aws_vpc" "default" {
  default = true
}

// Security group allowing all inbound traffic (for SSH, ICMP, etc.)
resource "aws_security_group" "allow_all" {
  name        = "fuoco-ephemeral-sg"
  description = "Allow all inbound and outbound traffic"
  vpc_id      = data.aws_vpc.default.id

  ingress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
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

resource "aws_instance" "vm" {
  ami           = data.aws_ssm_parameter.ami.value
  instance_type = var.instance_type
  user_data     = var.script
  vpc_security_group_ids = [aws_security_group.allow_all.id]
  tags = {
    Name = "fuoco-ephemeral"
  }
}

output "public_ip" {
  value = aws_instance.vm.public_ip
}