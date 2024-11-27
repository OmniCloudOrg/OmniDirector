#!/bin/bash

# Update package list
sudo apt-get update

# Install VirtualBox
sudo apt-get install -y virtualbox

# Install AWS CLI
sudo apt-get install -y awscli

# Install GCP CLI
echo "deb [signed-by=/usr/share/keyrings/cloud.google.gpg] http://packages.cloud.google.com/apt cloud-sdk main" | sudo tee -a /etc/apt/sources.list.d/google-cloud-sdk.list
sudo apt-get install -y apt-transport-https ca-certificates gnupg
curl https://packages.cloud.google.com/apt/doc/apt-key.gpg | sudo apt-key --keyring /usr/share/keyrings/cloud.google.gpg add -
sudo apt-get update && sudo apt-get install -y google-cloud-sdk

# Install govc CLI for vSphere
curl -L -o govc https://github.com/vmware/govmomi/releases/latest/download/govc_linux_amd64.gz
gunzip govc_linux_amd64.gz
chmod +x govc_linux_amd64
sudo mv govc_linux_amd64 /usr/local/bin/govc

# Install OpenStack CLI
sudo apt-get install -y python3-openstackclient

echo "Installation complete."