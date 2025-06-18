variable "token" {}
variable "instance_type" {}
variable "region" {}
variable "script_path" { default = "" }

provider "hcloud" {
  token    = var.token
  location = var.region
}

resource "hcloud_server" "vm" {
  name        = "fuoco-ephemeral"
  image       = "ubuntu-22.04"
  server_type = var.instance_type
  location    = var.region
  user_data   = var.script_path
}

output "public_ip" {
  value = hcloud_server.vm.ipv4_address
}

output "region" {
  value = var.region
}
