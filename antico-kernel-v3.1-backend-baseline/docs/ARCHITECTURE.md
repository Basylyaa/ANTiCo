# ANTICO Core 4-Pillar Architecture & Network Specifications (v3.1)

The ANTICO Knowledge Engine v3.1 is engineered on a resilient, high-concurrency, fully decoupled core model designed to operate within containerized boundaries (Podman/Docker) on host platforms like Ubuntu.

---

## The 4 Pillars of Resilient Architecture

### 1. Wildcard Socket Binding & Re-usability
The daemon server binds strictly to the wildcard IP `0.0.0.0:2020` (or `[::]:2020` for dual-stack support). To prevent socket-starvation issues and port locking during quick container/daemon recycles, the underlying Rust TCP socket explicitly configures:
- **`SO_REUSEADDR`**: Allows immediate reuse of local addresses in `TIME_WAIT` state.
- **`SO_REUSEPORT`**: Enables multiple sockets to bind to the same address/port combination without conflict.

### 2. Decoupled Non-blocking Execution
Long-running jobs (e.g., `bootstrap`, `node-register`, `promote`) are sent from the CLI or Proxy to the daemon as asynchronous commands. 
- The client receives a unique `job_id` immediately.
- The execution is spawned as an autonomous thread/task using Tokio.
- Disconnecting the client (network cut or `Ctrl+C`) **never** aborts or corrupts the active running task.

### 3. Stateless Presenter Model
The UI layer is designed as a pure, stateless presenter. It owns no operational state, database snapshots, or process logs. Instead, it interacts purely via standard polling and action dispatch APIs against the proxy proxy layer.

### 4. Memory-Resident Standby Logger
Stdout/stderr execution logging is statefully buffered within a thread-safe, memory-resident queue (`Arc<RwLock<Vec<Job>>>`) managed by the daemon on the background. Clients can re-attach at any moment to "hydrate" historical and live streaming outputs of complete or running operations.
