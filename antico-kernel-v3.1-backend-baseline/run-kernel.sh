#!/usr/bin/env bash
# run-kernel.sh: Build baseline container image and run under Podman with persistent volume mounting
set -euo pipefail

IMAGE_NAME="antico-kernel-backend:v3.1"
CONTAINER_NAME="antico-daemon"
DATA_HOST_DIR="/var/lib/antico/data"

echo "=== ANTICO (v3.1) Podman Execution Engine ==="

# 1. Create host data directory for persistent mounting
echo "--> Securing host data directory at ${DATA_HOST_DIR}..."
sudo mkdir -p "${DATA_HOST_DIR}"
sudo chmod 700 "${DATA_HOST_DIR}"

# 2. Build Container Image via Podman
echo "--> Building multi-stage Podman container image..."
podman build -t "${IMAGE_NAME}" -f Containerfile .

# 3. Stop and Remove existing container instance if present
if podman ps -a --format "{{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
    echo "--> Cleaning up existing active container '${CONTAINER_NAME}'..."
    podman stop "${CONTAINER_NAME}" || true
    podman rm "${CONTAINER_NAME}" || true
fi

# 4. Start Kernel Daemon under Podman
echo "--> Spawning detached background container with volume mounting..."
podman run -d \
    --name "${CONTAINER_NAME}" \
    --restart always \
    -p 2020:2020 \
    -v "${DATA_HOST_DIR}:/var/lib/antico/data:Z" \
    -e ANTICO_OWNER_TOKEN="${ANTICO_OWNER_TOKEN:-}" \
    "${IMAGE_NAME}"

echo "--> Container spawned successfully!"
echo "--> Listening on host port 2020 (0.0.0.0:2020)."
echo "--> Logs can be retrieved using: podman logs -f ${CONTAINER_NAME}"
