# ANTICO Engine Operations Runbook & Maintenance Manual (v3.1)

This runbook describes the procedure to build, deploy, execute, and monitor the ANTICO Knowledge Engine v3.1 backend baseline.

---

## 1. System Pre-requisites & OS Preparation

Perform network tuning and system packages installation:
```bash
chmod +x setup-deps.sh
./setup-deps.sh
```

---

## 2. Compiling Rust Binaries

Compile the workspace from source code:
```bash
cargo build --release
```
The compiled binary will be available at `./target/release/antico-cli`.

---

## 3. Running as a Native Systemd Service

1. Copy compiled binary to `/usr/local/bin/`:
   ```bash
   sudo cp ./target/release/antico-cli /usr/local/bin/
   ```
2. Set up data storage directories:
   ```bash
   sudo mkdir -p /var/lib/antico/data
   sudo chmod 700 /var/lib/antico/data
   ```
3. Install and start Systemd unit:
   ```bash
   sudo cp antico-kernel.service /etc/systemd/system/
   sudo systemctl daemon-reload
   sudo systemctl enable --now antico-kernel.service
   ```
4. Verify daemon status:
   ```bash
   sudo systemctl status antico-kernel.service
   ```

---

## 4. Running as a Podman Container

Deploy and configure with volume persistence and environment injection:
```bash
chmod +x run-kernel.sh
export ANTICO_OWNER_TOKEN="your_secure_admin_token"
./run-kernel.sh
```

Monitor daemon container outputs:
```bash
podman logs -f antico-daemon
```

---

## 5. Administrative Cli Actions

Execute privileged operations with token validation:

### Bootstrapping DB Hydration
```bash
antico-cli bootstrap --token <TOKEN_STRING> --file /app/data/bootstrap.json
```

### Self-Revoking Bootstraps
```bash
antico-cli bootstrap --token <TOKEN_STRING> --file /app/data/bootstrap.json --revoke-on-success
```

### Checking Daemon Jobs
```bash
antico-cli list-jobs
antico-cli view-logs <JOB_ID>
```
