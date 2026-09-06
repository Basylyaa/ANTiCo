# ANTICO Owner Token Security & Lifecycle Specification (v3.1)

To secure administrative commands, ANTICO implements a zero-hardcoding security protocol for privilege validation.

---

## 1. Zero-Hardcoding Dynamic Provisioning

The daemon supports two main strategies for booting with a security token:

### Environment Injection (Default)
When starting, the daemon inspects the `ANTICO_OWNER_TOKEN` environment variable. If present, this token is loaded directly into thread-safe memory state (`Arc<RwLock<Option<String>>>`).

### Auto-Generation (Fallback)
If no environment variable is present, the daemon automatically:
1. Generates a secure 32-character hex-encoded security token.
2. Writes the token string to the local disk at `/app/data/.owner_token` (fallback to relative `data/.owner_token` or `.owner_token` depending on execution paths).
3. Sets strict POSIX **`0600` permissions** (Read/Write only by owner) to avoid system credential leakage.

---

## 2. Authorization Layer

Every privileged action (`bootstrap`, `node-register`, `promote`) requires dynamic verification.

### CLI Token Verification
```bash
antico-cli bootstrap --token <OWNER_TOKEN>
```
The CLI serializes the token within the length-prefixed TCP binary frame.

### HTTP / REST REST API Verification
HTTP callers must submit credentials using standard authorization headers:
```http
Authorization: Bearer <OWNER_TOKEN>
```
The Express TCP Proxy extracts this header and forwards it directly to the daemon on port 2020.

---

## 3. Revocation Mechanics

Once verified, administrative tokens can be permanently revoked to secure the node against further modifications.

### Manual Revocation
Run the command:
```bash
antico-cli revoke-owner-token --token <OWNER_TOKEN>
```
This forces the daemon to:
1. Set the active token memory state to `None`.
2. Delete the physical token storage files (`.owner_token`).

### Automatic Post-Success Revocation
For automated setups, use:
```bash
antico-cli bootstrap --token <OWNER_TOKEN> --revoke-on-success
```
Upon successful hydration of database records (L1/L2/L3) in the detached background task, the daemon immediately revokes the active token from memory and disk, locking down the node for read-only operations automatically.
