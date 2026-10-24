//! Authenticated human preferences, never reusable execution capabilities.
#![forbid(unsafe_code)]
use serde::{Deserialize,Serialize};
use std::{path::{Path,PathBuf},sync::Mutex,io::{Read,Write}};
const MAX_RULES:usize=256;
const MAX_BYTES:u64=128*1024;
static SESSION:Mutex<Vec<(PathBuf,Rule)>>=Mutex::new(Vec::new());
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub tool:String,
    pub decision:String,
    pub scope:String,
    pub session_id:Option<String>,
    pub resource:Option<String>,
    pub context:Option<String>,
}
#[derive(Default,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {rules:Vec<Rule>}
pub fn path()->PathBuf {
    if let Some(path)=std::env::var_os("OPENCODE_RK_PERMISSION_RULES") {return path.into();}
    std::env::var_os("OPENCODE_RK_HOME").map(PathBuf::from)
        .unwrap_or_else(||PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share/opencode-rk"))
        .join("permission-rules.json")
}
pub fn fingerprint(text:&str)->String {blake3::hash(text.as_bytes()).to_hex().to_string()}
fn valid(rule:&Rule)->bool {
    (matches!(rule.tool.as_str(),"bash"|"shell"|"read"|"write"|"edit"|"glob"|"grep"|"webfetch"|"websearch"|"task"|"todowrite")
        || (rule.tool.starts_with("mcp_") && (5..=64).contains(&rule.tool.len())
            && rule.tool.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')))
        && matches!(rule.decision.as_str(),"allow"|"deny"|"ask")
        && matches!(rule.scope.as_str(),"session"|"always")
        && (rule.scope!="session" || rule.session_id.as_deref().is_some_and(|id|id.parse::<opencode_rk_contracts::SessionId>().is_ok()))
        && [rule.resource.as_deref(),rule.context.as_deref()].iter().all(|value|value.is_none_or(|v|v.len()==64&&v.bytes().all(|b|b.is_ascii_hexdigit())))
}
fn load(path:&Path)->Result<Vec<Rule>,String> {
    let meta=match std::fs::symlink_metadata(path) {Ok(meta)=>meta,Err(error) if error.kind()==std::io::ErrorKind::NotFound=>return Ok(Vec::new()),Err(_)=>return Err("permission rules unavailable".into())};
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len()>MAX_BYTES {return Err("unsafe permission rules file".into());}
    #[cfg(unix)] {use std::os::unix::fs::{PermissionsExt,MetadataExt};if meta.permissions().mode()&0o077!=0||meta.uid()!=rustix::process::geteuid().as_raw()||meta.nlink()!=1 {return Err("permission rules must be private and owned".into());}}
    let file=std::fs::File::open(path).map_err(|_|"permission rules unreadable")?;
    let opened=file.metadata().map_err(|_|"permission rules unavailable")?;
    #[cfg(unix)] {use std::os::unix::fs::MetadataExt;if opened.dev()!=meta.dev()||opened.ino()!=meta.ino() {return Err("permission rules changed during open".into());}}
    if !opened.is_file() {return Err("invalid permission file".into());}
    let mut raw=Vec::new();file.take(MAX_BYTES+1).read_to_end(&mut raw).map_err(|_|"permission rules unreadable")?;
    if raw.len()>MAX_BYTES as usize {return Err("permission rules too large".into());}
    let doc:Document=serde_json::from_slice(&raw).map_err(|_|"permission rules malformed")?;
    if doc.rules.len()>MAX_RULES || doc.rules.iter().any(|rule|!valid(rule)||rule.scope!="always") {return Err("invalid permission rules".into());}
    Ok(doc.rules)
}
fn save(path:&Path,rules:Vec<Rule>)->Result<(),String> {
    let bytes=serde_json::to_vec(&Document {rules}).map_err(|_|"permission encoding failed")?;
    if bytes.len()>MAX_BYTES as usize {return Err("permission rules too large".into());}
    let parent=path.parent().ok_or("invalid rules path")?;
    std::fs::create_dir_all(parent).map_err(|_|"permission directory unavailable")?;
    let temporary=parent.join(format!(".permission-rules-{}.tmp",opencode_rk_contracts::ApprovalId::new()));
    let result=(|| {
        let mut options=std::fs::OpenOptions::new();options.write(true).create_new(true);
        #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
        let mut file=options.open(&temporary).map_err(|_|"permission write failed")?;
        file.write_all(&bytes).map_err(|_|"permission write failed")?;
        file.sync_all().map_err(|_|"permission sync failed")?;
        std::fs::rename(&temporary,path).map_err(|_|"permission save failed")?;
        Ok(())
    })();
    if result.is_err() {let _=std::fs::remove_file(temporary);}
    result
}
pub fn list(session:Option<&str>)->Result<Vec<Rule>,String> {
    let path=path();let mut rules=load(&path)?;
    let sessions=SESSION.lock().map_err(|_|"permission rules busy")?;
    rules.extend(sessions.iter().filter(|(owner,rule)|owner==&path&&rule.session_id.as_deref()==session).map(|(_,rule)|rule.clone()));
    Ok(rules)
}
pub fn upsert(rule:Rule)->Result<(),String> {
    if !valid(&rule) {return Err("invalid tool, decision or scope".into());}
    let path=path();
    // One lock serializes both durable read/modify/write and session changes.
    let mut sessions=SESSION.lock().map_err(|_|"permission rules busy")?;
    let mut durable=load(&path)?;
    let same=|other:&Rule|other.tool==rule.tool&&other.scope==rule.scope&&other.session_id==rule.session_id
        && (rule.decision=="ask"&&rule.resource.is_none() || other.resource==rule.resource&&other.context==rule.context);
    if rule.scope=="always" {
        durable.retain(|other|!same(other));
        if rule.decision!="ask" {durable.push(rule);}
        if durable.len()>MAX_RULES {return Err("permission rule capacity reached".into());}
        save(&path,durable)
    } else {
        let count=sessions.iter().filter(|(owner,other)|owner!=&path||!same(other)).count();
        if count>=MAX_RULES && rule.decision!="ask" {return Err("session rule capacity reached".into());}
        sessions.retain(|(owner,other)|owner!=&path||!same(other));
        if rule.decision!="ask" {sessions.push((path,rule));}
        Ok(())
    }
}
pub fn decide(tool:&str,session:&str,resource:&str,context:&str)->Result<Option<bool>,String> {
    let rules=list(Some(session))?;
    let applicable=|rule:&&Rule|rule.tool==tool&&rule.resource.as_deref().is_none_or(|r|r==resource)
        && (rule.decision=="deny"||rule.context.as_deref().is_none_or(|c|c==context));
    if rules.iter().filter(applicable).any(|rule|rule.decision=="deny") {return Ok(Some(false));}
    Ok(rules.iter().filter(applicable).any(|rule|rule.decision=="allow").then_some(true))
}

pub fn denied(tool:&str,session:&str,intent:&opencode_rk_security::OperationIntent,broker:&opencode_rk_security::PermissionBroker)->bool {
    let Ok(resource)=serde_json::to_string(intent) else {return true;};
    !matches!(decide(tool,session,&fingerprint(&resource),&fingerprint(&format!("{:?}",broker.policy_context()))),Ok(None|Some(true)))
}
