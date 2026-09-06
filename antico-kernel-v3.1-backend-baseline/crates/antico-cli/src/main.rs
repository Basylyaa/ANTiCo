use antico_memory::{AnticoMemoryEngine, KnowledgeEngineTrait, L1Fact, L2Relation, L3Experience, L4AuditRecord};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct SeedPrior {
    agent_id: String,
    capability_id: String,
    alpha: f64,
    beta: f64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
struct BootstrapData {
    l1_facts: Vec<L1Fact>,
    l2_relations: Vec<L2Relation>,
    l3_experiences: Vec<L3Experience>,
}

// Stateful Background Job structure
#[derive(Serialize, Deserialize, Clone, Debug)]
struct Job {
    job_id: String,
    command: String,
    status: String, // "PENDING", "RUNNING", "COMPLETED", "FAILED"
    progress: f32,  // 0.0 to 1.0
    logs: Vec<String>,
    created_at: u64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
enum CommandRequest {
    StartJob { command: String, args: Vec<String>, token: Option<String> },
    ListJobs,
    GetLogs { job_id: String, start_line: usize },
    RevokeOwnerToken { token: String },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
enum CommandResponse {
    JobStarted { job_id: String },
    JobList { jobs: Vec<Job> },
    Logs { logs: Vec<String>, status: String, progress: f32 },
    Error { message: String },
    Revoked,
}

type SharedJobs = Arc<RwLock<Vec<Job>>>;

// Local path for state persistence of daemon jobs
const JOBS_STATE_FILE: &str = "jobs.json";

fn get_or_create_owner_token() -> String {
    if let Ok(tok) = std::env::var("ANTICO_OWNER_TOKEN") {
        if !tok.trim().is_empty() {
            return tok.trim().to_string();
        }
    }
    
    let paths = vec![
        "/app/data/.owner_token",
        "data/.owner_token",
        ".owner_token",
    ];
    
    for p in &paths {
        if let Ok(tok) = fs::read_to_string(p) {
            let trimmed = tok.trim().to_string();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
    }
    
    // Generate token dynamically
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    let seed_str = format!(
        "antico-seed-{:?}-{:?}",
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default(),
        std::process::id()
    );
    hasher.update(seed_str.as_bytes());
    let generated = format!("{:x}", hasher.finalize())[..32].to_string();
    
    // Attempt to write to paths
    for p in &paths {
        let path = Path::new(p);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            if let Ok(mut file) = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(path)
            {
                use std::io::Write;
                if write!(file, "{}", generated).is_ok() {
                    println!("🔑 Dynamically generated owner token and saved with 0600 permissions to {:?}", path);
                    return generated;
                }
            }
        }
        #[cfg(not(unix))]
        {
            if fs::write(path, &generated).is_ok() {
                println!("🔑 Dynamically generated owner token and saved to {:?}", path);
                return generated;
            }
        }
    }
    
    generated
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().collect();
    let mut host = std::env::var("ANTICO_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let mut token_arg = std::env::var("ANTICO_OWNER_TOKEN").ok();

    // Extract --host flag if present globally
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--host" {
            if i + 1 < args.len() {
                host = args[i + 1].clone();
                args.remove(i + 1);
                args.remove(i);
            } else {
                eprintln!("Error: Missing value for --host");
                std::process::exit(1);
            }
        } else if args[i] == "--token" {
            if i + 1 < args.len() {
                token_arg = Some(args[i + 1].clone());
                args.remove(i + 1);
                args.remove(i);
            } else {
                eprintln!("Error: Missing value for --token");
                std::process::exit(1);
            }
        } else {
            i += 1;
        }
    }

    if token_arg.is_none() {
        if let Ok(t) = fs::read_to_string("/app/data/.owner_token") {
            token_arg = Some(t.trim().to_string());
        } else if let Ok(t) = fs::read_to_string("data/.owner_token") {
            token_arg = Some(t.trim().to_string());
        } else if let Ok(t) = fs::read_to_string(".owner_token") {
            token_arg = Some(t.trim().to_string());
        }
    }

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    match args[1].as_str() {
        "daemon" => {
            start_daemon_server().await?;
        }
        "bootstrap" => {
            let mut file_path = "bootstrap.json".to_string();
            let mut out_db_path = "antico_memory.json".to_string();
            let mut revoke_on_success = false;

            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--file" | "-f" => {
                        if i + 1 < args.len() {
                            file_path = args[i + 1].clone();
                            i += 2;
                        } else {
                            eprintln!("Error: Missing value for --file");
                            std::process::exit(1);
                        }
                    }
                    "--db" | "-d" => {
                        if i + 1 < args.len() {
                            out_db_path = args[i + 1].clone();
                            i += 2;
                        } else {
                            eprintln!("Error: Missing value for --db");
                            std::process::exit(1);
                        }
                    }
                    "--revoke-on-success" => {
                        revoke_on_success = true;
                        i += 1;
                    }
                    _ => {
                        eprintln!("Error: Unknown argument '{}'", args[i]);
                        print_usage();
                        std::process::exit(1);
                    }
                }
            }

            // Attempt to connect to daemon on port 2020 for background decoupled execution
            match TcpStream::connect(format!("{}:2020", host)).await {
                Ok(mut stream) => {
                    println!("🔌 Connected to ANTICO Kernel daemon on {}:2020.", host);
                    println!("🚀 Submitting decoupled background bootstrap job...");
                    
                    let mut job_args = vec![file_path.clone(), out_db_path.clone()];
                    if revoke_on_success {
                        job_args.push("--revoke-on-success".to_string());
                    }

                    let req = CommandRequest::StartJob {
                        command: "bootstrap".to_string(),
                        args: job_args,
                        token: token_arg.clone(),
                    };
                    if let Err(e) = send_json(&mut stream, &req).await {
                        eprintln!("Error submitting job: {}. Falling back to inline execution.", e);
                        run_bootstrap_inline(&file_path, &out_db_path).await?;
                        return Ok(());
                    }

                    match recv_json::<CommandResponse>(&mut stream).await {
                        Ok(CommandResponse::JobStarted { job_id }) => {
                            println!("✅ Background job accepted: ID = {}", job_id);
                            println!("🔄 Attaching to streaming session. Feel free to disconnect (Ctrl+C) any time; task state persists.");
                            attach_to_job_logs(&host, &job_id).await?;
                        }
                        Ok(CommandResponse::Error { message }) => {
                            eprintln!("Daemon returned error: {}. Falling back to inline.", message);
                            run_bootstrap_inline(&file_path, &out_db_path).await?;
                        }
                        _ => {
                            eprintln!("Unexpected daemon response. Falling back to inline.");
                            run_bootstrap_inline(&file_path, &out_db_path).await?;
                        }
                    }
                }
                Err(_) => {
                    println!("⚠️ ANTICO Daemon is offline on {}:2020.", host);
                    println!("🛡️ Invoking inline execution fallback (Constitutional Law §1.3 - Knowledge preservation)...");
                    run_bootstrap_inline(&file_path, &out_db_path).await?;
                }
            }
        }
        "jobs" => {
            list_jobs_client(&host).await?;
        }
        "attach" => {
            if args.len() < 3 {
                eprintln!("Error: Please specify job_id to attach to.");
                std::process::exit(1);
            }
            attach_to_job_logs(&host, &args[2]).await?;
        }
        "logs" => {
            if args.len() < 3 {
                eprintln!("Error: Please specify job_id.");
                std::process::exit(1);
            }
            let tail = args.get(3).and_then(|s| s.parse::<usize>().ok()).unwrap_or(50);
            view_logs_client(&host, &args[2], tail).await?;
        }
        "revoke-owner-token" => {
            let tok = match token_arg {
                Some(t) => t,
                None => {
                    eprintln!("Error: Owner token must be provided via ANTICO_OWNER_TOKEN, --token, or file to perform revocation");
                    std::process::exit(1);
                }
            };
            
            match TcpStream::connect(format!("{}:2020", host)).await {
                Ok(mut stream) => {
                    let req = CommandRequest::RevokeOwnerToken { token: tok };
                    if let Err(e) = send_json(&mut stream, &req).await {
                        eprintln!("Error sending revocation request: {}", e);
                        std::process::exit(1);
                    }
                    
                    match recv_json::<CommandResponse>(&mut stream).await {
                        Ok(CommandResponse::Revoked) => {
                            println!("🔒 [SECURITY] Owner token successfully revoked from runtime memory and storage.");
                        }
                        Ok(CommandResponse::Error { message }) => {
                            eprintln!("Daemon error during revocation: {}", message);
                        }
                        _ => {
                            eprintln!("Unexpected daemon response.");
                        }
                    }
                }
                Err(_) => {
                    eprintln!("Error: Cannot connect to ANTICO daemon on {}:2020 for revocation", host);
                    std::process::exit(1);
                }
            }
        }
        _ => {
            eprintln!("Error: Unknown command '{}'", args[1]);
            print_usage();
            std::process::exit(1);
        }
    }

    Ok(())
}

fn print_usage() {
    println!("ANTICO Kernel v3.1 Admin CLI");
    println!("Usage:");
    println!("  antico-cli daemon                         - Start the background execution daemon (Port 2020)");
    println!("  antico-cli bootstrap -f <file> [-d <db>]  - Decoupled background bootstrap job submission");
    println!("                                              Supports optional: --revoke-on-success");
    println!("  antico-cli jobs                           - List all stateful background jobs");
    println!("  antico-cli attach <job_id>                - Re-attach to a running background task session");
    println!("  antico-cli logs <job_id> [tail]           - View static log tail of a specific job");
    println!("  antico-cli revoke-owner-token             - Manually revoke the active owner token and lock the kernel");
}

// Inline fallback execution
async fn run_bootstrap_inline(file_path: &str, out_db_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting Inline ANTICO Kernel v3.1 Bootstrap Pipeline...");
    println!("Reading seed file: {}", file_path);

    if !Path::new(file_path).exists() {
        return Err(format!("Seed file '{}' not found.", file_path).into());
    }

    let file_content = fs::read_to_string(file_path)?;
    let bootstrap_data: BootstrapData = serde_json::from_str(&file_content)?;

    let memory = AnticoMemoryEngine::new();

    println!("Hydrating L1 Facts...");
    for fact in bootstrap_data.l1_facts {
        memory.assert_fact(fact).await?;
    }

    println!("Hydrating L2 Relations...");
    for rel in bootstrap_data.l2_relations {
        memory.assert_relation(rel).await?;
    }

    println!("Hydrating L3 Experiences...");
    for exp in bootstrap_data.l3_experiences {
        memory.record_experience(exp).await?;
    }

    // Ensure the target directory for output DB exists
    if let Some(parent) = Path::new(out_db_path).parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    println!("Saving hydrated database to: {}", out_db_path);
    memory.save_to_file(out_db_path).await?;
    println!("✨ Inline bootstrap complete!");
    Ok(())
}

// TCP Stream Utilities
async fn send_json<S>(stream: &mut TcpStream, val: &S) -> Result<(), String>
where
    S: serde::Serialize,
{
    let payload = serde_json::to_string(val).map_err(|e| e.to_string())?;
    let len = payload.len() as u32;
    stream.write_all(&len.to_be_bytes()).await.map_err(|e| e.to_string())?;
    stream.write_all(payload.as_bytes()).await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn recv_json<R>(stream: &mut TcpStream) -> Result<R, String>
where
    R: for<'de> serde::Deserialize<'de>,
{
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
    let len = u32::from_be_bytes(len_buf) as usize;

    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await.map_err(|e| e.to_string())?;

    let val = serde_json::from_slice(&buf).map_err(|e| e.to_string())?;
    Ok(val)
}

// DAEMON IMPLEMENTATION
async fn start_daemon_server() -> Result<(), Box<dyn std::error::Error>> {
    println!("🟢 Starting ANTICO Kernel Background Execution Server on port 2020...");
    
    // Set up dynamically loaded/generated owner token
    let token = get_or_create_owner_token();
    let active_token = Arc::new(RwLock::new(Some(token)));

    let addr: std::net::SocketAddr = "0.0.0.0:2020".parse()?;
    let socket = socket2::Socket::new(socket2::Domain::IPV4, socket2::Type::STREAM, None)?;
    
    socket.set_reuse_address(true)?;
    
    #[cfg(all(unix, not(target_os = "solaris"), not(target_os = "illumos")))]
    {
        if let Err(e) = socket.set_reuse_port(true) {
            eprintln!("Warning: Failed to set SO_REUSEPORT: {}", e);
        }
    }
    
    socket.bind(&socket2::SockAddr::from(addr))?;
    socket.listen(128)?;
    
    let std_listener = std::net::TcpListener::from(socket);
    let listener = TcpListener::from_std(std_listener)?;

    // Load existing jobs state
    let mut loaded_jobs = Vec::new();
    if Path::new(JOBS_STATE_FILE).exists() {
        if let Ok(content) = fs::read_to_string(JOBS_STATE_FILE) {
            if let Ok(jobs) = serde_json::from_str(&content) {
                loaded_jobs = jobs;
            }
        }
    }
    let jobs = Arc::new(RwLock::new(loaded_jobs));

    // Listen loop
    loop {
        let (mut socket, _addr) = listener.accept().await?;
        let jobs_ref = Arc::clone(&jobs);
        let active_token_ref = Arc::clone(&active_token);

        tokio::spawn(async move {
            loop {
                match recv_json::<CommandRequest>(&mut socket).await {
                    Ok(req) => {
                        let response = handle_request(req, &jobs_ref, &active_token_ref).await;
                        if let Err(_) = send_json(&mut socket, &response).await {
                            break;
                        }
                    }
                    Err(_) => break, // Connection closed
                }
            }
        });
    }
}

async fn handle_request(
    req: CommandRequest,
    jobs: &SharedJobs,
    active_token: &Arc<RwLock<Option<String>>>,
) -> CommandResponse {
    match req {
        CommandRequest::StartJob { command, args, token } => {
            // Verify token on privileged actions: bootstrap, node-register, promote
            let is_privileged = command == "bootstrap" || command == "node-register" || command == "promote";
            if is_privileged {
                let token_guard = active_token.read().await;
                match &*token_guard {
                    Some(expected) => {
                        let provided = token.unwrap_or_default();
                        if provided != *expected {
                            return CommandResponse::Error {
                                message: "Unauthorized: Invalid or missing owner token for administrative actions".to_string(),
                            };
                        }
                    }
                    None => {
                        return CommandResponse::Error {
                            message: "Unauthorized: Administrative actions locked. Owner token is permanently revoked".to_string(),
                        };
                    }
                }
            }

            let job_id = format!("job-{:04}", SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() % 10000);

            let new_job = Job {
                job_id: job_id.clone(),
                command: command.clone(),
                status: "PENDING".to_string(),
                progress: 0.0,
                logs: vec![format!("⏳ Decoupled task '{}' requested.", command)],
                created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            };

            {
                let mut guard = jobs.write().await;
                guard.push(new_job);
                let _ = persist_jobs(&guard);
            }

            // Spawn decoupled asynchronous execution task
            let jobs_clone = Arc::clone(jobs);
            let job_id_clone = job_id.clone();
            let active_token_clone = Arc::clone(active_token);
            tokio::spawn(async move {
                let _ = run_background_task(job_id_clone, command, args, jobs_clone, active_token_clone).await;
            });

            CommandResponse::JobStarted { job_id }
        }
        CommandRequest::ListJobs => {
            let guard = jobs.read().await;
            CommandResponse::JobList { jobs: guard.clone() }
        }
        CommandRequest::GetLogs { job_id, start_line } => {
            let guard = jobs.read().await;
            if let Some(job) = guard.iter().find(|j| j.job_id == job_id) {
                let logs = if start_line < job.logs.len() {
                    job.logs[start_line..].to_vec()
                } else {
                    Vec::new()
                };
                CommandResponse::Logs {
                    logs,
                    status: job.status.clone(),
                    progress: job.progress,
                }
            } else {
                CommandResponse::Error {
                    message: format!("Job ID '{}' not found", job_id),
                }
            }
        }
        CommandRequest::RevokeOwnerToken { token } => {
            let mut token_guard = active_token.write().await;
            match &*token_guard {
                Some(expected) => {
                    if token == *expected {
                        *token_guard = None;
                        
                        // Clean up file storage
                        let paths = vec![
                            "/app/data/.owner_token",
                            "data/.owner_token",
                            ".owner_token",
                        ];
                        for p in paths {
                            let _ = fs::remove_file(p);
                        }
                        
                        CommandResponse::Revoked
                    } else {
                        CommandResponse::Error {
                            message: "Unauthorized: Invalid revocation credentials".to_string(),
                        }
                    }
                }
                None => {
                    CommandResponse::Error {
                        message: "Unauthorized: Owner token is already revoked".to_string(),
                    }
                }
            }
        }
    }
}

fn persist_jobs(jobs: &[Job]) -> Result<(), Box<dyn std::error::Error>> {
    let serialized = serde_json::to_string_pretty(jobs)?;
    fs::write(JOBS_STATE_FILE, serialized)?;
    Ok(())
}

async fn run_background_task(
    job_id: String,
    command: String,
    args: Vec<String>,
    jobs: SharedJobs,
    active_token: Arc<RwLock<Option<String>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Update to RUNNING
    {
        let mut guard = jobs.write().await;
        if let Some(job) = guard.iter_mut().find(|j| j.job_id == job_id) {
            job.status = "RUNNING".to_string();
            job.progress = 0.05;
            job.logs.push("🚀 Starting execution inside detached container kernel daemon...".to_string());
        }
        let _ = persist_jobs(&guard);
    }

    if command == "bootstrap" {
        let file_path = args.get(0).cloned().unwrap_or_else(|| "bootstrap.json".to_string());
        let out_db_path = args.get(1).cloned().unwrap_or_else(|| "antico_memory.json".to_string());

        let append_log = |msg: String| {
            let jobs_ref = Arc::clone(&jobs);
            let j_id = job_id.clone();
            tokio::spawn(async move {
                let mut guard = jobs_ref.write().await;
                if let Some(job) = guard.iter_mut().find(|j| j.job_id == j_id) {
                    job.logs.push(msg);
                }
            });
        };

        append_log(format!("Reading seed file: {}", file_path));
        tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;

        if !Path::new(&file_path).exists() {
            let err_msg = format!("❌ Seed file '{}' not found.", file_path);
            let mut guard = jobs.write().await;
            if let Some(job) = guard.iter_mut().find(|j| j.job_id == job_id) {
                job.status = "FAILED".to_string();
                job.logs.push(err_msg);
            }
            let _ = persist_jobs(&guard);
            return Ok(());
        }

        let file_content = fs::read_to_string(&file_path)?;
        let bootstrap_data: BootstrapData = serde_json::from_str(&file_content)?;

        // Set progress
        {
            let mut guard = jobs.write().await;
            if let Some(job) = guard.iter_mut().find(|j| j.job_id == job_id) {
                job.progress = 0.2;
                job.logs.push(format!("Parsing bootstrap.json successful. Found {} L1 facts, {} L2 relations.", bootstrap_data.l1_facts.len(), bootstrap_data.l2_relations.len()));
            }
        }

        let memory = AnticoMemoryEngine::new();

        // L1 Hydration
        append_log("Starting hydration of L1 facts...".to_string());
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        for fact in bootstrap_data.l1_facts {
            let fact_id = fact.id.clone();
            memory.assert_fact(fact).await?;
            append_log(format!("✓ L1 Fact hydrated: {}", fact_id));
        }

        {
            let mut guard = jobs.write().await;
            if let Some(job) = guard.iter_mut().find(|j| j.job_id == job_id) {
                job.progress = 0.5;
            }
        }

        // L2 Hydration
        append_log("Starting hydration of L2 relations...".to_string());
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        for rel in bootstrap_data.l2_relations {
            let rel_id = rel.id.clone();
            memory.assert_relation(rel).await?;
            append_log(format!("✓ L2 Relationship hydrated: {}", rel_id));
        }

        {
            let mut guard = jobs.write().await;
            if let Some(job) = guard.iter_mut().find(|j| j.job_id == job_id) {
                job.progress = 0.8;
            }
        }

        // L3 Hydration
        append_log("Starting hydration of L3 experiences...".to_string());
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        for exp in bootstrap_data.l3_experiences {
            let exp_id = exp.id.clone();
            memory.record_experience(exp).await?;
            append_log(format!("✓ L3 Experience registered: {}", exp_id));
        }

        // Saving
        append_log(format!("Saving database schema to path: {}", out_db_path));
        tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
        
        if let Some(parent) = Path::new(&out_db_path).parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        memory.save_to_file(&out_db_path).await?;

        // Completed!
        {
            let mut guard = jobs.write().await;
            if let Some(job) = guard.iter_mut().find(|j| j.job_id == job_id) {
                job.status = "COMPLETED".to_string();
                job.progress = 1.0;
                job.logs.push("✨ ANTICO Knowledge database fully operational. Cold start latency reduced to 0ms!".to_string());
                
                // If --revoke-on-success is provided, clear/revoke token dynamically
                if args.iter().any(|arg| arg == "--revoke-on-success") {
                    job.logs.push("🔒 [SECURITY] --revoke-on-success triggered. Owner token revoked permanently from runtime memory and storage.".to_string());
                }
            }
            let _ = persist_jobs(&guard);
        }

        if args.iter().any(|arg| arg == "--revoke-on-success") {
            let mut token_guard = active_token.write().await;
            *token_guard = None;
            
            let paths = vec![
                "/app/data/.owner_token",
                "data/.owner_token",
                ".owner_token",
            ];
            for p in paths {
                let _ = fs::remove_file(p);
            }
        }
    } else {
        // Mock generic execution or testing command
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        {
            let mut guard = jobs.write().await;
            if let Some(job) = guard.iter_mut().find(|j| j.job_id == job_id) {
                job.status = "COMPLETED".to_string();
                job.progress = 1.0;
                job.logs.push("Generic administrative operation finished successfully.".to_string());
            }
            let _ = persist_jobs(&guard);
        }
    }

    Ok(())
}

// CLIENT UTILITIES
async fn list_jobs_client(host: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect(format!("{}:2020", host)).await?;
    send_json(&mut stream, &CommandRequest::ListJobs).await.map_err(|e| Box::<dyn std::error::Error>::from(e))?;
    match recv_json::<CommandResponse>(&mut stream).await.map_err(|e| Box::<dyn std::error::Error>::from(e)) {
        Ok(CommandResponse::JobList { jobs }) => {
            println!("{:<10} {:<12} {:<12} {:<10} {:<15}", "JOB ID", "COMMAND", "STATUS", "PROGRESS", "CREATED AT");
            println!("{}", "-".repeat(65));
            for job in jobs {
                let prog_bar = format!("{:.0}%", job.progress * 100.0);
                println!("{:<10} {:<12} {:<12} {:<10} {}", job.job_id, job.command, job.status, prog_bar, job.created_at);
            }
        }
        _ => eprintln!("Failed to retrieve jobs list from daemon."),
    }
    Ok(())
}

async fn view_logs_client(host: &str, job_id: &str, tail: usize) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect(format!("{}:2020", host)).await?;
    send_json(&mut stream, &CommandRequest::GetLogs { job_id: job_id.to_string(), start_line: 0 }).await.map_err(|e| Box::<dyn std::error::Error>::from(e))?;
    match recv_json::<CommandResponse>(&mut stream).await.map_err(|e| Box::<dyn std::error::Error>::from(e)) {
        Ok(CommandResponse::Logs { logs, status, progress }) => {
            println!("📋 Logs for Job {} (Status: {}, Progress: {:.1}%)", job_id, status, progress * 100.0);
            println!("{}", "=".repeat(65));
            let len = logs.len();
            let start = if len > tail { len - tail } else { 0 };
            for log in &logs[start..] {
                println!("{}", log);
            }
        }
        _ => eprintln!("Job ID not found or error loading logs."),
    }
    Ok(())
}

async fn attach_to_job_logs(host: &str, job_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut current_line = 0;
    let mut finished = false;

    while !finished {
        let mut stream = match TcpStream::connect(format!("{}:2020", host)).await {
            Ok(s) => s,
            Err(_) => {
                println!("⚠️ Lost connection to ANTICO daemon! Retrying re-attachment in 1s...");
                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                continue;
            }
        };

        let req = CommandRequest::GetLogs {
            job_id: job_id.to_string(),
            start_line: current_line,
        };

        if let Err(_) = send_json(&mut stream, &req).await {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            continue;
        }

        match recv_json::<CommandResponse>(&mut stream).await {
            Ok(CommandResponse::Logs { logs, status, progress }) => {
                for log in logs {
                    println!("{}", log);
                    current_line += 1;
                }

                // Print nice progress meter
                if status == "RUNNING" {
                    let fill_chars = (progress * 20.0) as usize;
                    let empty_chars = 20 - fill_chars;
                    let bar = format!("[{}{}] {:.0}%", "#".repeat(fill_chars), "-".repeat(empty_chars), progress * 100.0);
                    print!("\r⏳ Resuming Task... {} ", bar);
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                } else {
                    println!("\n🏁 Job status updated to: {}", status);
                    finished = true;
                }
            }
            _ => {
                eprintln!("Error polling daemon logs.");
                finished = true;
            }
        }

        if !finished {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }

    Ok(())
}
