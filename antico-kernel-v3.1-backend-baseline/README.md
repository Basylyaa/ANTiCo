# ANTICO Knowledge Engine & Daemon Baseline Package (v3.1)

A clean, high-performance, container-ready backend baseline containing exclusively the Rust core workspace, orchestration tooling, and administrative CLI utilities — completely independent of any UI or Node.js frontend.

---

## Directory Layout
```plaintext
antico-kernel-v3.1-backend-baseline/
├── Cargo.toml                  # Clean Rust Workspace definition
├── Containerfile               # Multi-stage Rust build & runtime specification
├── setup-deps.sh               # Ubuntu socket tuning & Podman dependency installer
├── run-kernel.sh               # Volume mounting & Podman execution
├── antico-kernel.service       # Native Systemd unit configuration
├── crates/                     # Pure Backend Crates
│   ├── antico-cli/             # Command-line interface & status manager
│   ├── antico-kernel/          # Core async daemon primitives (Port 2020)
│   ├── antico-gateway/         # TCP binary framing engine
│   └── antico-verification/    # A-T68 Golden Acceptance Test suite
└── docs/
    ├── ARCHITECTURE.md         # 4-Pillars & Socket Architecture Docs
    ├── OWNER_TOKEN_SPEC.md     # Owner Token dynamic lifecycle docs
    └── RUNBOOK.md              # Operational manual
```

---

## Quickstart Guide

### 1. Tune & Install Dependencies (Ubuntu)
```bash
chmod +x setup-deps.sh
./setup-deps.sh
```

### 2. Standalone Build & Workspace Testing
Run all workspace unit tests and the A-T68 Golden Suite:
```bash
cargo test --workspace
```

### 3. Deploy under Podman
Build the multi-stage image and run the detached container bound to `0.0.0.0:2020` with persistent storage mounted to `/var/lib/antico/data`:
```bash
chmod +x run-kernel.sh
export ANTICO_OWNER_TOKEN="super_secure_token"
./run-kernel.sh
```

### 4. Admin Operations via CLI
```bash
# Execute bootstrap hydration
cargo run --bin antico-cli -- bootstrap --token super_secure_token --revoke-on-success
```

Refer to `/docs/RUNBOOK.md` for complete operational, maintenance, and diagnostics instructions.
