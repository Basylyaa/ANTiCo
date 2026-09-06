#!/usr/bin/env bash
# setup-deps.sh: Ubuntu dependency installer and network socket tuning suite for ANTICO (v3.1)
set -euo pipefail

echo "=== ANTICO (v3.1) Core System Optimizer and Podman dependency installer ==="

# 1. Update APT and Install Podman & Systemd Utilities
echo "--> Installing baseline system packages..."
sudo apt-get update
sudo apt-get install -y podman systemd libssl-dev build-essential

# 2. Network TCP Socket Optimization Tuning
echo "--> Enforcing system-wide TCP socket tuning configurations..."
# Enable TCP reuse of TIME_WAIT sockets for fast port recovery
sudo sysctl -w net.ipv4.tcp_tw_reuse=1
# Optimize socket buffer memory boundaries
sudo sysctl -w net.core.rmem_max=16777216
sudo sysctl -w net.core.wmem_max=16777216
sudo sysctl -w net.ipv4.tcp_rmem="4096 87380 16777216"
sudo sysctl -w net.ipv4.tcp_wmem="4096 65536 16777216"
# Raise maximum system-wide open file handles to support concurrent connections
sudo sysctl -w fs.file-max=2097152

# Persist sysctl parameters across reboots
cat <<EOF | sudo tee /etc/sysctl.d/99-antico-tune.conf
net.ipv4.tcp_tw_reuse = 1
net.core.rmem_max = 16777216
net.core.wmem_max = 16777216
net.ipv4.tcp_rmem = 4096 87380 16777216
net.ipv4.tcp_wmem = 4096 65536 16777216
fs.file-max = 2097152
EOF

echo "--> System TCP socket configurations applied and persisted!"
echo "=== Baseline Setup Completed Successfully ==="
