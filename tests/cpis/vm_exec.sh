#!/bin/bash

# Configuration variables
worker_NAME="OmniVM"
MEMORY_MB=2048
CPU_COUNT=2
OS_TYPE="Ubuntu_64"
BOX_PATH="$HOME/Downloads/trusty-server-cloudimg-amd64-vagrant-disk1.box"
EXTRACTED_DIR="/tmp/vagrant_box_extracted"
API_ENDPOINT="http://localhost:8081/vms/action"

echo "Extracting Vagrant box..."
mkdir -p "$EXTRACTED_DIR"
tar -xf "$BOX_PATH" -C "$EXTRACTED_DIR"

echo "Finding virtual disk file..."
DISK_PATH=""
for disk in "$EXTRACTED_DIR"/*.vmdk "$EXTRACTED_DIR"/*.vdi "$EXTRACTED_DIR"/*.vhd; do
    if [ -f "$disk" ]; then
        DISK_PATH="$disk"
        echo "Found disk: $DISK_PATH"
        break
    fi
done

if [ -z "$DISK_PATH" ]; then
    echo "Error: No disk file found in the extracted box."
    exit 1
fi

echo "Creating VM..."
cat > /tmp/worker_request.json << EOF
{
  "provider": "virtualbox_cpi_linux",
  "action": "create_vm",
  "params": {
    "worker_name": "$worker_NAME",
    "os_type": "$OS_TYPE",
    "memory_mb": $MEMORY_MB,
    "cpu_count": $CPU_COUNT
  }
}
EOF

curl -s -X POST -H "Content-Type: application/json" -d @"/tmp/worker_request.json" "$API_ENDPOINT"

echo "Attaching disk..."
cat > /tmp/attach_disk.json << EOF
{
  "provider": "virtualbox_cpi_linux",
  "action": "attach_disk",
  "params": {
    "worker_name": "$worker_NAME",
    "controller_name": "SATAController",
    "port": "0",
    "disk_path": "$DISK_PATH"
  }
}
EOF

curl -s -X POST -H "Content-Type: application/json" -d @"/tmp/attach_disk.json" "$API_ENDPOINT"

echo "Starting VM..."
cat > /tmp/start_vm.json << EOF
{
  "provider": "virtualbox_cpi_linux",
  "action": "start_vm",
  "params": {
    "worker_name": "$worker_NAME"
  }
}
EOF

curl -s -X POST -H "Content-Type: application/json" -d @"/tmp/start_vm.json" "$API_ENDPOINT"

echo "Waiting 90 seconds for VM to boot..."
sleep 1

echo "Executing command in VM..."
cat > /tmp/execute_command.json << EOF
{
  "provider": "virtualbox_cpi_linux",
  "action": "execute_command",
  "params": {
    "worker_name": "$worker_NAME",
    "username": "vagrant",
    "password": "vagrant",
    "command_path": "/bin/uname",
    "command_args": "-a"
  }
}
EOF

echo "Debug: Executing command JSON:"
cat /tmp/execute_command.json
echo ""

curl -v -X POST -H "Content-Type: application/json" -d @"/tmp/execute_command.json" "$API_ENDPOINT"
echo "Script completed."