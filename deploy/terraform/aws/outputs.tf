output "public_ip" {
  value = aws_instance.loopflow.public_ip
}

output "ssh" {
  value = "ssh ubuntu@${aws_instance.loopflow.public_ip}"
}

output "repo_dir" {
  value = var.repo_dir
}
