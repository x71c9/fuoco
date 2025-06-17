variable "token" {}
variable "instance_type" {}
variable "location" {}
variable "script" { default = "" }

provider "hcloud" {
  token    = var.token
  location = var.location
}

resource "hcloud_server" "vm" {
  name        = "fuoco-ephemeral"
  image       = "ubuntu-22.04"
  server_type = var.instance_type
  location    = var.location
  user_data   = var.script
}

output "ipv4_address" {
  value = hcloud_server.vm.ipv4_address
}