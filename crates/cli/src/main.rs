#![forbid(unsafe_code)]
use clap::{Args, Parser, Subcommand, ValueEnum};
use opencode_rk_catalog::{Catalog, CatalogQuery};
use opencode_rk_contracts::{
    CapabilityReport, DiagnosticReport, MessageRole, SessionId, WIRE_SCHEMA_VERSION,
};
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{Storage, StoragePaths};
use opencode_rk_tools::registry::ToolRegistry;
use serde::Serialize;
use std::{env, fs, net::SocketAddr, path::PathBuf, str::FromStr, sync::Arc};
const MODELS_DEV_URL: &str = "https://models.dev/api.json";
#[derive(Debug, Parser)]
#[command(
    name = "opencode-rk",
    version,
    about = "Resource-efficient native coding-agent harness"
)]
struct Cli {
    #[arg(long, global = true, env = "OPENCODE_RK_HOME")]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
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
        Command::Doctor(args) => doctor(args).await?,
        Command::Session { command } => {
            let data = resolve_data_dir(cli.data_dir)?;
            let sessions = open_sessions(data)?;
            session_command(&sessions, command).await?;
        }
        Command::Models { command } => {
            let data = resolve_data_dir(cli.data_dir)?;
            model_command(data, command).await?;
        }
        Command::Serve(args) => {
            let data = resolve_data_dir(cli.data_dir)?;
            serve(data, args).await?;
        }
    }
    Ok(())
}
#[derive(Debug, Serialize)]
struct DoctorCheck {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
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
            os_sandbox_backend: None,
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
        println!("native core: yes\nembedded sqlite: yes\njavascript compatibility host: disabled\nos sandbox: not yet implemented");
        println!("auth: {}", output.checks.auth.status);
        println!("connectivity: {}", output.checks.connectivity.status);
        println!("tools: {}", output.checks.tools.status);
        println!("mcp: {}", output.checks.mcp.status);
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
    DoctorCheck {
        status: if configured {
            "configured"
        } else {
            "unconfigured"
        },
        detail: None,
        builtins: Vec::new(),
    }
}

async fn doctor_connectivity_check() -> DoctorCheck {
    let Some(endpoint) = env::var_os("OPENCODE_RK_DOCTOR_ENDPOINT") else {
        return DoctorCheck {
            status: "unconfigured",
            detail: None,
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
                builtins: Vec::new(),
            };
        }
    };
    match client.get(&endpoint).send().await {
        Ok(response) if response.status().is_success() => DoctorCheck {
            status: "ok",
            detail: Some(response.status().as_u16().to_string()),
            builtins: Vec::new(),
        },
        Ok(response) => DoctorCheck {
            status: "error",
            detail: Some(format!("HTTP {}", response.status().as_u16())),
            builtins: Vec::new(),
        },
        Err(error) => DoctorCheck {
            status: "error",
            detail: Some(error.to_string()),
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
        builtins,
    }
}

fn doctor_mcp_check() -> DoctorCheck {
    let Some(raw) = env::var_os("OPENCODE_RK_MCP_CONFIG") else {
        return DoctorCheck {
            status: "unconfigured",
            detail: None,
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
        builtins: Vec::new(),
    }
}
fn open_sessions(data: PathBuf) -> Result<SessionService, Box<dyn std::error::Error>> {
    Ok(SessionService::new(Arc::new(Storage::open(
        StoragePaths::under(data),
    )?)))
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
async fn serve(data: PathBuf, args: ServeArgs) -> Result<(), Box<dyn std::error::Error>> {
    let sessions = open_sessions(data.clone())?;
    let path = args
        .models_file
        .unwrap_or_else(|| catalog_cache_path(&data));
    let catalog = if path.exists() {
        Arc::new(Catalog::from_models_dev_api_json(&fs::read(path)?)?)
    } else {
        Arc::new(Catalog::default())
    };
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    tracing::info!(listen=%args.listen,"native server listening");
    axum::serve(listener, router(AppState { sessions, catalog })).await?;
    Ok(())
}
fn catalog_cache_path(data: &std::path::Path) -> PathBuf {
    data.join("catalog/models.dev.api.json")
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
