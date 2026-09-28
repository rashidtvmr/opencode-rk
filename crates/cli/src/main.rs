#![forbid(unsafe_code)]
use clap::{Args, Parser, Subcommand, ValueEnum};
use opencode_rk_catalog::{Catalog, CatalogQuery};
use opencode_rk_contracts::{
    CapabilityReport, DiagnosticReport, MessageRole, SessionId, WIRE_SCHEMA_VERSION,
};
use opencode_rk_server::{
    app_runtime::EnginePolicy,
    daemon::{
        publish_backend_descriptor_with_auth, read_backend_descriptor, BackendDescriptor,
        DaemonError, DaemonPaths, SingletonDaemon,
    },
    daemon_auth::DaemonAuth,
    router_with_auth,
    runtime_wiring::{EngineLease, RuntimeWiring},
    AppState,
};
use opencode_rk_sessions::{SessionManager, SessionService};
use opencode_rk_storage::{Storage, StoragePaths};
use opencode_rk_tools::registry::ToolRegistry;
use serde::Serialize;
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::{env, fs, io::Read, net::SocketAddr, path::PathBuf, str::FromStr, sync::Arc};
mod ci_output;
mod ci_run;
mod composer;
mod tui_entry;
use tui_entry::TuiArgs;
mod app_start;
mod chat;
mod daemon_client;
mod diagnostics;
mod graph;
mod headless_engine;
mod install_commands;
mod medown;
mod modals;
mod native_app;
mod native_approvals;
mod native_composer;
mod native_host;
mod native_layout;
mod native_navigation;
mod native_palette;
mod native_shell;
mod native_status;
mod native_theme;
mod native_timeline;
mod native_transcript;
mod onboarding;
mod pair;
mod service_commands;
mod shutdown;
mod terminal_host;
mod themes;
mod title;
mod transcript;
mod tui_paint;
const MODELS_DEV_URL: &str = "https://models.dev/api.json";
#[derive(Debug, Parser)]
#[command(
    name = "oc2",
    version,
    about = "Resource-efficient native coding-agent harness"
)]
struct Cli {
    #[arg(long, global = true, env = "OPENCODE_RK_HOME")]
    data_dir: Option<PathBuf>,
    /// Run the native OpenTUI renderer (requires vendored libopentui).
    #[arg(long, global = true)]
    native: bool,
    /// Render one bounded frame and exit (top-level alias for `tui --once`).
    /// Non-global: the `tui` subcommand keeps its own `--once`.
    #[arg(long)]
    once: bool,
    /// No subcommand opens the interactive chat TUI.
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Debug, Subcommand)]
enum Command {
    Doctor(DoctorArgs),
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    Models {
        #[command(subcommand)]
        command: ModelCommand,
    },
    Serve(ServeArgs),
    Web(WebArgs),
    Tui(TuiArgs),
    Run(RunArgs),
}
#[derive(Debug, Args)]
struct RunArgs {
    /// Enable CI non-interactive mode with JSONL event output.
    #[arg(long)]
    ci: bool,
    /// Output format for CI mode.
    #[arg(long, default_value = "jsonl")]
    output: String,
    /// Maximum number of turn iterations (0 = usage error).
    #[arg(long)]
    max_steps: Option<u64>,
    /// Wall-clock timeout in seconds for the entire CI run (0 = usage error).
    #[arg(long)]
    timeout: Option<u64>,
    /// The prompt to execute (use "doctor" for machine-readable health checks).
    #[arg(long)]
    prompt: Option<String>,
    /// The prompt to execute as a positional (use "doctor" for health checks).
    prompt_pos: Option<String>,
}
#[derive(Debug, Args)]
struct DoctorArgs {
    #[arg(long)]
    json: bool,
}
#[derive(Debug, Args)]
struct ServeArgs {
    #[arg(long, default_value = "127.0.0.1:4096")]
    listen: SocketAddr,
    #[arg(long)]
    models_file: Option<PathBuf>,
}
#[derive(Debug, Args)]
struct WebArgs {
    #[arg(long, default_value = "127.0.0.1:4096")]
    listen: SocketAddr,
    #[arg(long)]
    models_file: Option<PathBuf>,
    #[arg(long)]
    no_open: bool,
}
#[derive(Debug, Subcommand)]
enum SessionCommand {
    Create {
        title: String,
    },
    List {
        #[arg(long)]
        all: bool,
    },
    Show {
        id: String,
    },
    Rename {
        id: String,
        title: String,
    },
    Archive {
        id: String,
    },
    Message {
        #[command(subcommand)]
        command: MessageCommand,
    },
}
#[derive(Debug, Subcommand)]
enum MessageCommand {
    Add {
        session_id: String,
        #[arg(long, value_enum)]
        role: RoleArg,
        text: String,
    },
    List {
        session_id: String,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
}
#[derive(Clone, Copy, Debug, ValueEnum)]
enum RoleArg {
    System,
    User,
    Assistant,
    Tool,
}
impl From<RoleArg> for MessageRole {
    fn from(v: RoleArg) -> Self {
        match v {
            RoleArg::System => Self::System,
            RoleArg::User => Self::User,
            RoleArg::Assistant => Self::Assistant,
            RoleArg::Tool => Self::Tool,
        }
    }
}
#[derive(Debug, Subcommand)]
enum ModelCommand {
    Sync {
        #[arg(long,default_value=MODELS_DEV_URL)]
        url: String,
    },
    Search {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        query: Option<String>,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        reasoning: bool,
        #[arg(long)]
        tools: bool,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
}
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    let cli = Cli::parse();
    if let Err(e) = run(cli).await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        None => {
            use std::io::IsTerminal as _;
            let probe = app_start::TtyProbe {
                stdin_is_tty: Some(std::io::stdin().is_terminal()),
                stdout_is_tty: Some(std::io::stdout().is_terminal()),
            };
            let data = resolve_data_dir(cli.data_dir)?;
            let presence = daemon_client::discover_presence(&data);
            let creds = daemon_client::creds_configured(&data);
            let plan = app_start::plan_default_launch(&probe, presence, creds);
            match plan.mode {
                app_start::LaunchMode::NativeTui => {
                    // A native-enabled release defaults to the real OpenTUI
                    // path. Development builds without the native feature keep
                    // the compatibility chat unless --native is explicit.
                    if cli.native || cfg!(feature = "native") {
                        let lease = chat::prepare_daemon(&data);
                        let args = TuiArgs {
                            once: cli.once,
                            origin: lease.origin().map(str::to_owned),
                            session: None,
                            follow: false,
                            follow_for: None,
                            model: "openai/gpt-5.6".to_string(),
                            reasoning_effort: "high".to_string(),
                            poll_ms: 1000,
                            submit_keymap: None,
                            memory: vec![],
                        };
                        tui_entry::run_with_dir(args, Some(&data))?;
                    } else {
                        chat::run(&data)?;
                    }
                }
                app_start::LaunchMode::Headless(reason) => {
                    eprintln!("error: {}", app_start::headless_message(reason));
                    std::process::exit(app_start::HEADLESS_EXIT_CODE);
                }
                app_start::LaunchMode::Error(error) => {
                    eprintln!("error: {error}");
                    std::process::exit(2);
                }
            }
        }
        Some(Command::Doctor(args)) => doctor(args).await?,
        Some(Command::Session { command }) => {
            let data = resolve_data_dir(cli.data_dir)?;
            let sessions = open_sessions(data)?;
            session_command(&sessions, command).await?;
        }
        Some(Command::Models { command }) => {
            let data = resolve_data_dir(cli.data_dir)?;
            model_command(data, command).await?;
        }
        Some(Command::Serve(args)) => {
            let data = resolve_data_dir(cli.data_dir)?;
            serve(data, args, false).await?;
        }
        Some(Command::Web(args)) => {
            let data = resolve_data_dir(cli.data_dir)?;
            web(data, args).await?;
        }
        Some(Command::Tui(mut args)) => {
            let data = resolve_data_dir(cli.data_dir)?;
            // An explicit origin wins. Otherwise the TUI shares the exact
            // singleton-daemon bootstrap used by the default application.
            let lease = if args.origin.is_none() {
                let lease = chat::prepare_daemon(&data);
                args.origin = lease.origin().map(str::to_owned);
                Some(lease)
            } else {
                None
            };
            tui_entry::run_with_dir(args, Some(&data))?;
            drop(lease);
        }
        Some(Command::Run(args)) => {
            // Prompt via positional or --prompt; absent only matters for usage
            // validation below (ci_run treats a missing prompt as an error).
            let prompt = args
                .prompt
                .clone()
                .or_else(|| args.prompt_pos.clone())
                .unwrap_or_default();
            if !args.ci {
                eprintln!("error: --ci flag is required for non-interactive CI mode");
                std::process::exit(ci_output::CiExitCode::UsageError as i32);
            }
            // Validate max-steps and timeout: 0 is a typed usage error. These
            // exit before clap finalizes so `run --ci --max-steps 0` (no
            // prompt) is a typed 64 rather than a clap 2.
            if args.max_steps == Some(0) {
                eprintln!("error: --max-steps must be > 0");
                std::process::exit(ci_output::CiExitCode::UsageError as i32);
            }
            if args.timeout == Some(0) {
                eprintln!("error: --timeout must be > 0");
                std::process::exit(ci_output::CiExitCode::UsageError as i32);
            }
            if prompt.is_empty() {
                eprintln!("error: a prompt is required (positional or --prompt)");
                std::process::exit(ci_output::CiExitCode::UsageError as i32);
            }
            let format = match args.output.as_str() {
                "json" | "jsonl" => ci_output::OutputFormat::Jsonl,
                "text" => ci_output::OutputFormat::Text,
                _ => ci_output::OutputFormat::Jsonl,
            };
            let mut stdout = std::io::stdout();
            let result = ci_run::run_ci(&prompt, format, &mut stdout, args.max_steps, args.timeout);
            if result.exit_code != 0 {
                std::process::exit(result.exit_code as i32);
            }
        }
    }
    Ok(())
}
#[derive(Debug, Serialize)]
struct DoctorCheck {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    /// Concrete remediation hint for unconfigured/error checks: names the
    /// env var to set or the command to run. Absent when healthy.
    #[serde(skip_serializing_if = "Option::is_none")]
    next_step: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    builtins: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DoctorChecks {
    auth: DoctorCheck,
    connectivity: DoctorCheck,
    tools: DoctorCheck,
    mcp: DoctorCheck,
}

#[derive(Debug, Serialize)]
struct DoctorOutput {
    #[serde(flatten)]
    report: DiagnosticReport,
    checks: DoctorChecks,
}

async fn doctor(args: DoctorArgs) -> Result<(), Box<dyn std::error::Error>> {
    let report = DiagnosticReport {
        schema_version: WIRE_SCHEMA_VERSION,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        capabilities: CapabilityReport {
            native_core: true,
            embedded_sqlite: true,
            js_compatibility_host: false,
            os_sandbox_backend: Some(diagnostics::sandbox_backend_name().to_owned()),
            feature_profile: "foundation".to_owned(),
        },
    };
    let checks = DoctorChecks {
        auth: doctor_auth_check(),
        connectivity: doctor_connectivity_check().await,
        tools: doctor_tools_check(),
        mcp: doctor_mcp_check(),
    };
    let output = DoctorOutput { report, checks };
    if args.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("OpenCode RK {}", output.report.version);
        println!(
            "native core: yes\nembedded sqlite: yes\njavascript compatibility host: disabled\nos sandbox: {}",
            diagnostics::sandbox_backend_name()
        );
        println!("auth: {}", output.checks.auth.status);
        println!("connectivity: {}", output.checks.connectivity.status);
        println!("tools: {}", output.checks.tools.status);
        println!("mcp: {}", output.checks.mcp.status);
        let hint = |label: &str, check: &DoctorCheck| {
            if let Some(step) = &check.next_step {
                println!("  {label} next step: {step}");
            }
        };
        hint("auth", &output.checks.auth);
        hint("connectivity", &output.checks.connectivity);
        hint("mcp", &output.checks.mcp);
    }
    Ok(())
}

fn doctor_auth_check() -> DoctorCheck {
    const AUTH_ENV_KEYS: &[&str] = &[
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
        "GOOGLE_API_KEY",
        "GEMINI_API_KEY",
    ];
    let configured = AUTH_ENV_KEYS
        .iter()
        .any(|key| env::var_os(key).is_some_and(|value| !value.is_empty()));
    let next_step = (!configured).then(|| {
        format!(
            "set a provider key, e.g. export OPENAI_API_KEY=sk-... (also accepted: {})",
            AUTH_ENV_KEYS[1..].join(", ")
        )
    });
    DoctorCheck {
        status: if configured {
            "configured"
        } else {
            "unconfigured"
        },
        detail: None,
        next_step,
        builtins: Vec::new(),
    }
}

async fn doctor_connectivity_check() -> DoctorCheck {
    let Some(endpoint) = env::var_os("OPENCODE_RK_DOCTOR_ENDPOINT") else {
        return DoctorCheck {
            status: "unconfigured",
            detail: None,
            next_step: Some(
                "set OPENCODE_RK_DOCTOR_ENDPOINT to an https URL to probe provider reachability"
                    .to_owned(),
            ),
            builtins: Vec::new(),
        };
    };
    let endpoint = endpoint.to_string_lossy().into_owned();
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(750))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return DoctorCheck {
                status: "error",
                detail: Some(error.to_string()),
                next_step: Some("check OPENCODE_RK_DOCTOR_ENDPOINT is a valid URL".to_owned()),
                builtins: Vec::new(),
            };
        }
    };
    match client.get(&endpoint).send().await {
        Ok(response) if response.status().is_success() => DoctorCheck {
            status: "ok",
            detail: Some(response.status().as_u16().to_string()),
            next_step: None,
            builtins: Vec::new(),
        },
        Ok(response) => DoctorCheck {
            status: "error",
            detail: Some(format!("HTTP {}", response.status().as_u16())),
            next_step: Some("verify the endpoint URL and your network".to_owned()),
            builtins: Vec::new(),
        },
        Err(error) => DoctorCheck {
            status: "error",
            detail: Some(error.to_string()),
            next_step: Some("verify the endpoint URL and your network".to_owned()),
            builtins: Vec::new(),
        },
    }
}

fn doctor_tools_check() -> DoctorCheck {
    let mut builtins = ToolRegistry::new()
        .list()
        .into_iter()
        .map(|tool| tool.id.clone())
        .collect::<Vec<_>>();
    builtins.sort();
    DoctorCheck {
        status: "ok",
        detail: None,
        next_step: None,
        builtins,
    }
}

fn doctor_mcp_check() -> DoctorCheck {
    let Some(raw) = env::var_os("OPENCODE_RK_MCP_CONFIG") else {
        return DoctorCheck {
            status: "unconfigured",
            detail: None,
            next_step: Some(
                "set OPENCODE_RK_MCP_CONFIG to a JSON document like {\"servers\":{\"name\":{\"command\":\"...\"}}} to attach MCP servers"
                    .to_owned(),
            ),
            builtins: Vec::new(),
        };
    };
    let raw = raw.to_string_lossy();
    let status = serde_json::from_str::<serde_json::Value>(&raw)
        .ok()
        .and_then(|value| {
            value
                .get("servers")
                .and_then(|servers| servers.as_object())
                .cloned()
        })
        .filter(|servers| !servers.is_empty())
        .map_or("error", |_| "configured");
    DoctorCheck {
        status,
        detail: None,
        next_step: (status == "error").then(|| {
            "OPENCODE_RK_MCP_CONFIG must be JSON with a non-empty {\"servers\":{...}} object"
                .to_owned()
        }),
        builtins: Vec::new(),
    }
}
fn open_sessions(data: PathBuf) -> Result<SessionService, Box<dyn std::error::Error>> {
    Ok(SessionService::new(Arc::new(Storage::open(
        StoragePaths::under(data),
    )?)))
}
fn open_web_sessions(data: &std::path::Path) -> Result<SessionService, Box<dyn std::error::Error>> {
    let storage = Arc::new(Storage::open(StoragePaths::under(data.to_path_buf()))?);
    let branch_path = data.join("workspaces/local/web-branches-v2.db");
    let branch_manager = Arc::new(SessionManager::open_branch_workspace(&branch_path)?);
    let sessions = SessionService::with_branch_manager(storage, branch_manager);
    let catalog_path = data.join("catalog.db");
    if catalog_path.exists() {
        Ok(sessions.with_workspace_catalog_path(&catalog_path)?)
    } else {
        Ok(sessions)
    }
}
async fn session_command(
    s: &SessionService,
    c: SessionCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match c {
        SessionCommand::Create { title } => {
            println!("{}", serde_json::to_string_pretty(&s.create(title).await?)?)
        }
        SessionCommand::List { all } => {
            println!("{}", serde_json::to_string_pretty(&s.list(all).await?)?)
        }
        SessionCommand::Show { id } => {
            let id = SessionId::from_str(&id)?;
            println!("{}", serde_json::to_string_pretty(&s.get(id).await?)?);
        }
        SessionCommand::Rename { id, title } => {
            let id = SessionId::from_str(&id)?;
            s.rename(id, title).await?;
            println!("{}", serde_json::to_string_pretty(&s.get(id).await?)?);
        }
        SessionCommand::Archive { id } => {
            let id = SessionId::from_str(&id)?;
            s.archive(id).await?;
            println!("{}", serde_json::to_string_pretty(&s.get(id).await?)?);
        }
        SessionCommand::Message { command } => match command {
            MessageCommand::Add {
                session_id,
                role,
                text,
            } => {
                let id = SessionId::from_str(&session_id)?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&s.append_text(id, role.into(), text).await?)?
                );
            }
            MessageCommand::List { session_id, limit } => {
                let id = SessionId::from_str(&session_id)?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&s.messages(id, limit).await?)?
                );
            }
        },
    }
    Ok(())
}
async fn model_command(data: PathBuf, c: ModelCommand) -> Result<(), Box<dyn std::error::Error>> {
    match c {
        ModelCommand::Sync { url } => {
            let bytes = reqwest::get(&url)
                .await?
                .error_for_status()?
                .bytes()
                .await?;
            Catalog::from_models_dev_api_json(&bytes)?;
            let path = catalog_cache_path(&data);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, &bytes)?;
            println!("{}", path.display());
        }
        ModelCommand::Search {
            file,
            query,
            provider,
            reasoning,
            tools,
            limit,
        } => {
            let bytes = fs::read(file.unwrap_or_else(|| catalog_cache_path(&data)))?;
            let catalog = Catalog::from_models_dev_api_json(&bytes)?;
            let results = catalog.search(&CatalogQuery {
                text: query,
                provider,
                reasoning: reasoning.then_some(true),
                tools: tools.then_some(true),
                limit,
                ..CatalogQuery::default()
            })?;
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
    }
    Ok(())
}

/// Bounded wait for the singleton owner to publish its authenticated
/// descriptor. The `serve` AlreadyRunning path calls this: the PID lock is
/// held by another process, so this caller must never bind a second
/// listener. `Ok(None)` (absent, stale pid, schema mismatch, or non-loopback
/// origin) is retryable until the monotonic deadline; `Err`
/// (symlink/owner/oversize/malformed-or-empty-token/oversize-after-read)
/// fails closed immediately and is propagated, never mapped to absence.
/// `Ok(None)` is returned only at the deadline.
async fn wait_for_owner_descriptor(
    data: &std::path::Path,
) -> Result<Option<BackendDescriptor>, DaemonError> {
    const WAIT_BUDGET: std::time::Duration = std::time::Duration::from_millis(2000);
    const WAIT_POLL: std::time::Duration = std::time::Duration::from_millis(25);
    let deadline = std::time::Instant::now() + WAIT_BUDGET;
    loop {
        match read_backend_descriptor(data)? {
            Some(descriptor) => return Ok(Some(descriptor)),
            None => {}
        }
        if std::time::Instant::now() >= deadline {
            return Ok(None);
        }
        tokio::time::sleep(WAIT_POLL).await;
    }
}

async fn web(data: PathBuf, args: WebArgs) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(descriptor) = read_backend_descriptor(&data)? {
        println!("{}", descriptor.http_origin);
        if !args.no_open {
            open_web_browser(&descriptor.http_origin)?;
        }
        return Ok(());
    }

    serve(
        data,
        ServeArgs {
            listen: args.listen,
            models_file: args.models_file,
        },
        !args.no_open,
    )
    .await
}

const MAX_CONFIG_BYTES: usize = 64 * 1024;
const MAX_CONFIG_DEPTH: usize = 64;
const MAX_CONFIG_NODES: usize = 8192;
const MAX_CONFIG_CONTAINER_ITEMS: usize = 4096;

fn invalid_config() -> Box<dyn std::error::Error> {
    std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid configuration").into()
}

fn config_file_path(directory: &std::path::Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let json = directory.join("opencode.json");
    match fs::symlink_metadata(&json) {
        Ok(_) => Ok(json),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(directory.join("opencode.jsonc"))
        }
        Err(_) => Err(invalid_config()),
    }
}

fn normalize_jsonc(input: &str) -> Result<String, Box<dyn std::error::Error>> {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Normal,
        String,
        LineComment,
        BlockComment,
    }

    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut state = State::Normal;
    let mut escaped = false;
    let mut depth = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        match state {
            State::String => {
                output.push(byte);
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    state = State::Normal;
                }
                index += 1;
            }
            State::LineComment => {
                if byte == b'\n' || byte == b'\r' {
                    output.push(byte);
                    state = State::Normal;
                } else {
                    output.push(b' ');
                }
                index += 1;
            }
            State::BlockComment => {
                if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    output.extend_from_slice(b"  ");
                    state = State::Normal;
                    index += 2;
                } else {
                    output.push(if byte == b'\n' || byte == b'\r' {
                        byte
                    } else {
                        b' '
                    });
                    index += 1;
                }
            }
            State::Normal => match byte {
                b'"' => {
                    output.push(byte);
                    state = State::String;
                    index += 1;
                }
                b'/' if bytes.get(index + 1) == Some(&b'/') => {
                    output.extend_from_slice(b"  ");
                    state = State::LineComment;
                    index += 2;
                }
                b'/' if bytes.get(index + 1) == Some(&b'*') => {
                    output.extend_from_slice(b"  ");
                    state = State::BlockComment;
                    index += 2;
                }
                b',' => {
                    let mut next = index + 1;
                    loop {
                        while matches!(bytes.get(next), Some(b' ' | b'\t' | b'\r' | b'\n')) {
                            next += 1;
                        }
                        if bytes.get(next..next + 2) == Some(b"//") {
                            next += 2;
                            while !matches!(bytes.get(next), None | Some(b'\n' | b'\r')) {
                                next += 1;
                            }
                        } else if bytes.get(next..next + 2) == Some(b"/*") {
                            let Some(end) = bytes[next + 2..]
                                .windows(2)
                                .position(|window| window == b"*/")
                            else {
                                break;
                            };
                            next += end + 4;
                        } else {
                            break;
                        }
                    }
                    if matches!(bytes.get(next), Some(b']' | b'}')) {
                        index += 1;
                    } else {
                        output.push(byte);
                        index += 1;
                    }
                }
                b'{' | b'[' => {
                    depth += 1;
                    if depth > MAX_CONFIG_DEPTH {
                        return Err(invalid_config());
                    }
                    output.push(byte);
                    index += 1;
                }
                b'}' | b']' => {
                    depth = depth.saturating_sub(1);
                    output.push(byte);
                    index += 1;
                }
                _ => {
                    output.push(byte);
                    index += 1;
                }
            },
        }
    }
    if state == State::String || state == State::BlockComment {
        return Err(invalid_config());
    }
    String::from_utf8(output).map_err(|_| invalid_config())
}

fn validate_config_value(value: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    fn visit(
        value: &JsonValue,
        depth: usize,
        nodes: &mut usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        *nodes = nodes.checked_add(1).ok_or_else(invalid_config)?;
        if *nodes > MAX_CONFIG_NODES || depth > MAX_CONFIG_DEPTH {
            return Err(invalid_config());
        }
        match value {
            JsonValue::Array(items) => {
                if items.len() > MAX_CONFIG_CONTAINER_ITEMS {
                    return Err(invalid_config());
                }
                for item in items {
                    visit(item, depth + 1, nodes)?;
                }
            }
            JsonValue::Object(items) => {
                if items.len() > MAX_CONFIG_CONTAINER_ITEMS {
                    return Err(invalid_config());
                }
                for item in items.values() {
                    visit(item, depth + 1, nodes)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    visit(value, 0, &mut 0)
}

fn parse_config(bytes: &[u8]) -> Result<JsonValue, Box<dyn std::error::Error>> {
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err(invalid_config());
    }
    let source = std::str::from_utf8(bytes).map_err(|_| invalid_config())?;
    let normalized = normalize_jsonc(source)?;
    let value: JsonValue = serde_json::from_str(&normalized).map_err(|_| invalid_config())?;
    if !value.is_object() {
        return Err(invalid_config());
    }
    validate_config_value(&value)?;
    Ok(value)
}

fn merge_config(target: &mut JsonValue, incoming: JsonValue) {
    match (target, incoming) {
        (JsonValue::Object(target), JsonValue::Object(incoming)) => {
            for (key, value) in incoming {
                match target.get_mut(&key) {
                    Some(existing) => merge_config(existing, value),
                    None => {
                        target.insert(key, value);
                    }
                }
            }
        }
        (target, incoming) => *target = incoming,
    }
}

fn load_config_directory(
    directory: &std::path::Path,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    // JSONC is the documented alternate and takes precedence if both names exist.
    let path = config_file_path(directory)?;
    let path_metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(JsonValue::Object(JsonMap::new()));
        }
        Err(_) => return Err(invalid_config()),
    };
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(invalid_config());
    }
    let mut file = fs::File::open(&path).map_err(|_| invalid_config())?;
    let mut bytes = Vec::with_capacity(MAX_CONFIG_BYTES.min(4096));
    file.by_ref()
        .take((MAX_CONFIG_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid_config())?;
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err(invalid_config());
    }
    parse_config(&bytes)
}

fn load_effective_config(
    data: &std::path::Path,
    cwd: &std::path::Path,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let mut merged = load_config_directory(data)?;
    merge_config(&mut merged, load_config_directory(cwd)?);
    validate_config_value(&merged)?;
    Ok(merged)
}

fn project_custom_providers(config: &JsonValue) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let source = config.get("provider").or_else(|| config.get("providers"));
    let Some(source) = source else {
        return Ok(JsonValue::Object(JsonMap::new()));
    };
    let providers = source.as_object().ok_or_else(invalid_config)?;
    let mut result = JsonMap::new();
    for (provider_id, provider) in providers {
        let mut projected = provider.as_object().cloned().ok_or_else(invalid_config)?;
        if !projected.get("name").is_some_and(JsonValue::is_string) {
            return Err(invalid_config());
        }
        let models = projected
            .get_mut("models")
            .and_then(JsonValue::as_object_mut)
            .ok_or_else(invalid_config)?;
        for (model_key, model) in models.iter_mut() {
            let model = model.as_object_mut().ok_or_else(invalid_config)?;
            let id = model
                .entry("id".to_owned())
                .or_insert_with(|| JsonValue::String(model_key.clone()));
            if !id.is_string() {
                return Err(invalid_config());
            }
        }
        projected.insert("id".to_owned(), JsonValue::String(provider_id.clone()));
        result.insert(provider_id.clone(), JsonValue::Object(projected));
    }
    Ok(JsonValue::Object(result))
}

fn merged_catalog_json(
    path: &std::path::Path,
    config: &JsonValue,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut catalog = if path.exists() {
        let metadata = fs::metadata(path)?;
        if metadata.len() > 16 * 1024 * 1024 {
            return Err("models.dev catalog exceeds 16 MiB".into());
        }
        let bytes = fs::read(path)?;
        let parsed: JsonValue = serde_json::from_slice(&bytes)?;
        if !parsed.is_object() {
            return Err("models.dev catalog must be an object".into());
        }
        parsed
    } else {
        JsonValue::Object(JsonMap::new())
    };
    let configured = project_custom_providers(config)?;
    let catalog = catalog.as_object_mut().ok_or_else(invalid_config)?;
    for (provider_id, provider) in configured.as_object().ok_or_else(invalid_config)? {
        catalog.insert(provider_id.clone(), provider.clone());
    }
    serde_json::to_vec(&catalog).map_err(Into::into)
}

async fn serve(
    data: PathBuf,
    args: ServeArgs,
    open_browser: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let effective_config = load_effective_config(&data, &cwd)?;
    let path = args
        .models_file
        .unwrap_or_else(|| catalog_cache_path(&data));
    let catalog_json = merged_catalog_json(&path, &effective_config)?;
    let catalog = Arc::new(Catalog::from_models_dev_api_json(&catalog_json)?);
    let daemon_paths = DaemonPaths::for_data_dir(&data);
    let daemon = match SingletonDaemon::bind(&daemon_paths.socket, &daemon_paths.pid) {
        Ok(daemon) => Arc::new(daemon),
        Err(DaemonError::AlreadyRunning(_)) => {
            // The PID lock is held by the owner, which may still be
            // publishing its descriptor. Poll boundedly for the valid
            // authenticated descriptor instead of failing one-shot; a
            // symlink/owner/oversize/malformed-or-empty-token refusal
            // (`Err`) propagates immediately, and no second listener is
            // ever bound here.
            let descriptor = wait_for_owner_descriptor(&data).await?.ok_or_else(|| {
                "backend is already running but its endpoint descriptor is unavailable".to_string()
            })?;
            println!("{}", descriptor.http_origin);
            if open_browser {
                open_web_browser(&descriptor.http_origin)?;
            }
            return Ok(());
        }
        Err(error) => return Err(Box::new(error)),
    };
    let sessions = open_web_sessions(&data)?;
    let _engine_lease = EngineLease::acquire()?;
    let mut runtime_policy = EnginePolicy::default_deny();
    for name in env::var("OPENCODE_RK_TURN_TOOLS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        runtime_policy.allow_tool(name.to_owned())?;
    }
    let runtime = RuntimeWiring::for_daemon(sessions.clone(), ToolRegistry::new(), runtime_policy);
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    let listen = listener.local_addr()?;
    // Integration fix (wave-3): the RC-01-hardened `read_backend_descriptor`
    // treats an empty `auth_token` as a stale/legacy descriptor, so a serve
    // published via the no-auth variant was invisible to its own reuse path
    // ("backend is already running but its endpoint descriptor is
    // unavailable"). Mint the real daemon credential and publish it.
    //
    let credential = DaemonAuth::mint().map_err(|error| error.to_string())?;
    let descriptor =
        publish_backend_descriptor_with_auth(&data, listen, credential.token().to_owned())?;
    println!("{}", descriptor.http_origin);
    if open_browser {
        open_web_browser(&descriptor.http_origin)?;
    }
    tracing::info!(listen=%listen,"native singleton server listening");
    let daemon_accept = Arc::clone(&daemon);
    let control = tokio::spawn(async move {
        daemon_accept.accept_clients().await;
    });
    let app = router_with_auth(AppState { sessions, catalog }, Some(credential))
        .layer(axum::Extension(runtime));
    let result = axum::serve(listener, app).await;
    daemon.shutdown();
    let _ = control.await;
    result?;
    Ok(())
}
fn open_web_browser(origin: &str) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", "", origin]);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = std::process::Command::new("open");
        command.arg(origin);
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(origin);
        command
    };
    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    return Err("opening a browser is unsupported on this platform; use --no-open".into());

    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}
fn catalog_cache_path(data: &std::path::Path) -> PathBuf {
    data.join("catalog/models.dev.api.json")
}

#[cfg(test)]
mod config_ingest_tests {
    use super::*;

    #[test]
    fn jsonc_comments_trailing_commas_preserve_string_bytes() {
        let source = r#"{"url":"https://example.test/a//b","text":"comma, } // /* still text","items":[1,2,],/* removed */"ok":true,}"#;
        let normalized = normalize_jsonc(source).expect("normalize valid JSONC");
        let escaped_source = r#"{"escape":"quote: \" // still string"}"#;
        let escaped = normalize_jsonc(escaped_source).expect("normalize escaped string");
        assert_eq!(
            serde_json::from_str::<JsonValue>(&escaped).expect("parse escaped string")["escape"],
            "quote: \" // still string"
        );
        let value: JsonValue = serde_json::from_str(&normalized).expect("parse normalized JSON");
        assert_eq!(value["url"], "https://example.test/a//b");
        assert_eq!(value["text"], "comma, } // /* still text");
        assert_eq!(value["items"], serde_json::json!([1, 2]));
    }

    #[test]
    fn project_config_deep_merges_over_global_and_array_replaces() {
        let root = std::env::temp_dir().join(format!("config-ingest-{}", std::process::id()));
        let data = root.join("data");
        let project = root.join("project");
        fs::create_dir_all(&data).expect("create global config dir");
        fs::create_dir_all(&project).expect("create project config dir");
        fs::write(
            data.join("opencode.json"),
            br#"{"provider":{"acme":{"options":{"headers":{"a":"global","b":"keep"},"body":{"global":true}},"env":["A","B"]}}}"#,
        )
        .expect("write global config");
        fs::write(
            project.join("opencode.json"),
            br#"{"provider":{"acme":{"options":{"headers":{"a":"project"}},"env":["C"]}}}"#,
        )
        .expect("write project config");
        let config = load_effective_config(&data, &project).expect("merge global then project");
        assert_eq!(
            config["provider"]["acme"]["options"]["headers"]["a"],
            "project"
        );
        assert_eq!(
            config["provider"]["acme"]["options"]["headers"]["b"],
            "keep"
        );
        assert_eq!(
            config["provider"]["acme"]["options"]["body"]["global"],
            true
        );
        assert_eq!(config["provider"]["acme"]["env"], serde_json::json!(["C"]));
        fs::remove_dir_all(root).expect("remove config fixture");
    }

    #[test]
    fn json_config_wins_when_both_config_names_exist() {
        let root = std::env::temp_dir().join(format!("config-precedence-{}", std::process::id()));
        fs::create_dir_all(&root).expect("create config dir");
        fs::write(root.join("opencode.json"), b"{}").expect("write JSON config");
        fs::write(root.join("opencode.jsonc"), b"{}").expect("write JSONC config");
        assert_eq!(
            config_file_path(&root).expect("select JSON"),
            root.join("opencode.json")
        );
        fs::remove_dir_all(root).expect("remove config fixture");
    }

    #[test]
    fn oversized_and_deep_config_rejected() {
        assert!(parse_config(&vec![b' '; MAX_CONFIG_BYTES + 1]).is_err());
        assert!(validate_config_value(&JsonValue::Array(vec![
            JsonValue::Null;
            MAX_CONFIG_CONTAINER_ITEMS + 1
        ]))
        .is_err());
        let oversized_map: JsonMap<String, JsonValue> = (0..=MAX_CONFIG_NODES)
            .map(|index| (index.to_string(), JsonValue::Null))
            .collect();
        assert!(validate_config_value(&JsonValue::Object(oversized_map)).is_err());
        let nested = format!(
            "{}0{}",
            "[".repeat(MAX_CONFIG_DEPTH + 1),
            "]".repeat(MAX_CONFIG_DEPTH + 1)
        );
        assert!(parse_config(nested.as_bytes()).is_err());
        assert!(normalize_jsonc(r#"{"x":/* unfinished"#).is_err());
    }

    #[test]
    fn provider_projection_keeps_configured_keys_and_wire_model_id() {
        let config: JsonValue = serde_json::from_str(
            r#"{"provider":{"acme":{"name":"Acme","npm":"@ai-sdk/openai-compatible","env":["FIRST","SECOND"],"options":{"headers":{"x":"y"},"body":{"k":1}},"models":{"model-key":{"name":"Model","id":"wire-model","extra":true}}}}}"#,
        )
        .expect("provider config");
        let projected = project_custom_providers(&config).expect("project provider");
        assert_eq!(projected["acme"]["id"], "acme");
        assert_eq!(
            projected["acme"]["env"],
            serde_json::json!(["FIRST", "SECOND"])
        );
        assert_eq!(projected["acme"]["options"]["headers"]["x"], "y");
        assert_eq!(projected["acme"]["options"]["body"]["k"], 1);
        assert_eq!(projected["acme"]["models"]["model-key"]["id"], "wire-model");
        assert_eq!(projected["acme"]["models"]["model-key"]["extra"], true);
    }
}

fn resolve_data_dir(override_dir: Option<PathBuf>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(path) = override_dir {
        return Ok(path);
    }
    if let Some(home) = env::var_os("HOME") {
        return Ok(PathBuf::from(home).join(".local/share/opencode-rk"));
    }
    if let Some(home) = env::var_os("USERPROFILE") {
        return Ok(PathBuf::from(home).join(".opencode-rk"));
    }
    Err("cannot determine data directory; pass --data-dir or set OPENCODE_RK_HOME".into())
}
