variable "project" {}
variable "zone" {}
variable "instance_type" {}
variable "script" { default = "" }

provider "google" {
  project = var.project
  zone    = var.zone
}

// Use latest Ubuntu 20.04 LTS image family (dynamic alias)
data "google_compute_image" "ubuntu" {
  family  = "ubuntu-2004-lts"
  project = "ubuntu-os-cloud"
}

resource "google_compute_instance" "vm" {
  name         = "fuoco-ephemeral"
  machine_type = var.instance_type
  boot_disk {
    initialize_params {
      image = data.google_compute_image.ubuntu.self_link
    }
  }
  network_interface {
    network = "default"
    access_config {}
  }
  metadata_startup_script = var.script
}

output "external_ip" {
  value = google_compute_instance.vm.network_interface[0].access_config[0].nat_ip
}