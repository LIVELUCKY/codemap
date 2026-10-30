variable "region" {
  default = "eu-central-1"
}

resource "local_file" "assets" {
  filename = var.region
}

module "network" {
  source = "./modules/network"
}

output "bucket_name" {
  value = local_file.assets.id
}
