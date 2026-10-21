//! Bounded, metadata-only application event journal. Never record input text,
//! provider credentials, HTTP bodies or tool arguments here.
use serde_json::{json, Value};
use std::{fs::{File, OpenOptions}, io::Write, path::{Path, PathBuf}, sync::{Mutex, OnceLock}, time::{SystemTime, UNIX_EPOCH}};

const MAX_FILE: u64 = 8 * 1024 * 1024;
const MAX_EVENT: usize = 4096;
static JOURNAL: OnceLock<Mutex<Journal>> = OnceLock::new();

struct Journal { file: File, path: PathBuf, bytes: u64, sequence: u64 }

fn now_ms() -> u128 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() }

fn open(path: &Path) -> std::io::Result<File> {
    let _directory = directory_lock(path.parent().ok_or_else(|| std::io::Error::other("missing journal directory"))?)?;
    open_locked(path)
}

fn directory_lock(path: &Path) -> std::io::Result<File> {
    let directory = File::open(path)?;
    #[cfg(unix)]
    rustix::fs::flock(&directory, rustix::fs::FlockOperation::LockExclusive)?;
    Ok(directory)
}

fn open_locked(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    let file = options.open(path)?;
    #[cfg(unix)]
    rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive)?;
    Ok(file)
}

pub fn init(data: &Path, role: &str) -> std::io::Result<PathBuf> {
    let directory = data.join("logs");
    std::fs::create_dir_all(&directory)?;
    prune(&directory)?;
    let path = directory.join(format!("{role}-{}-{}.jsonl", std::process::id(), now_ms()));
    let file = open(&path)?;
    let _ = JOURNAL.set(Mutex::new(Journal { file, path: path.clone(), bytes: 0, sequence: 0 }));
    event("app.start", json!({"role":role,"version":env!("CARGO_PKG_VERSION")}));
    Ok(path)
}

fn prune(directory: &Path) -> std::io::Result<()> {
    // Coordinate pathname operations with creation/rotation, not just inode
    // ownership. Otherwise a rename between open and unlink can remove a
    // different writer's replacement file.
    let _directory = directory_lock(directory)?;
    let mut previous: Vec<_> = std::fs::read_dir(&directory)?.filter_map(Result::ok).filter_map(|entry| {
        let name = entry.file_name(); let name = name.to_str()?;
        if !(name.starts_with("client-") || name.starts_with("daemon-")) || !(name.ends_with(".jsonl") || name.ends_with(".jsonl.1") || name.ends_with(".jsonl.2")) { return None; }
        let metadata = entry.metadata().ok()?;
        metadata.is_file().then(|| (metadata.modified().ok(), entry.path()))
    }).collect();
    previous.sort_by_key(|entry| entry.0);
    let mut excess = previous.len().saturating_sub(32);
    for (_, path) in previous {
        if excess == 0 { break; }
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        #[cfg(unix)] {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
        }
        let Ok(file) = options.open(&path) else { continue };
        if !file.metadata()?.is_file() { continue; }
        // The writer holds this advisory lock for the descriptor's lifetime,
        // including idle daemons. No PID/mtime heuristic can prove inactivity.
        #[cfg(unix)]
        if rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive).is_err() { continue; }
        #[cfg(not(unix))]
        continue;
        if std::fs::remove_file(path).is_ok() { excess -= 1; }
    }
    Ok(())
}

pub fn event(name: &str, fields: Value) {
    if let Some(journal) = JOURNAL.get() {
        if let Ok(mut journal) = journal.lock() { let _ = journal.record(name, fields); }
    }
}

impl Journal {
    fn record(&mut self, name: &str, fields: Value) -> std::io::Result<()> {
        if !self.path.try_exists()? {
            self.file = open(&self.path)?;
            self.bytes = 0;
        }
        self.sequence += 1;
        let record = json!({"time_ms":now_ms(),"pid":std::process::id(),"sequence":self.sequence,"event":name,"fields":fields});
        let mut bytes = serde_json::to_vec(&record)?;
        if bytes.len() > MAX_EVENT { bytes = serde_json::to_vec(&json!({"time_ms":now_ms(),"pid":std::process::id(),"sequence":self.sequence,"event":name,"fields":{"omitted":"event exceeded metadata budget"}}))?; }
        bytes.push(b'\n');
        if self.bytes + bytes.len() as u64 > MAX_FILE {
            let _directory = directory_lock(self.path.parent().ok_or_else(|| std::io::Error::other("missing journal directory"))?)?;
            // Each process owns one journal and at most two rotated files.
            self.file.flush()?;
            let older = self.path.with_extension("jsonl.2");
            let previous = self.path.with_extension("jsonl.1");
            if older.exists() { std::fs::remove_file(&older)?; }
            if previous.exists() { std::fs::rename(&previous, &older)?; }
            std::fs::rename(&self.path, &previous)?;
            self.file = open_locked(&self.path)?;
            self.bytes = 0;
        }
        self.file.write_all(&bytes)?;
        self.bytes += bytes.len() as u64;
        Ok(())
    }
}

pub struct TraceWriter(Vec<u8>);
pub fn trace_writer() -> TraceWriter { TraceWriter(Vec::new()) }
impl Write for TraceWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let available = MAX_EVENT.saturating_sub(self.0.len());
        self.0.extend_from_slice(&bytes[..bytes.len().min(available)]);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}
impl Drop for TraceWriter {
    fn drop(&mut self) {
        // Foreign tracing messages may include HTTP bodies or credentials.
        // The application journal records typed protocol facts separately.
        if !self.0.is_empty() { event("runtime.trace", json!({"bytes":self.0.len(),"message_omitted":true})); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_debug_records_parseable_ordered_bounded_events() {
        let dir = std::env::temp_dir().join("opencode").join(format!("oc2-event-test-{}-{}",std::process::id(),now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("events.jsonl");
        let mut journal = Journal { file:open(&path).unwrap(), path:path.clone(), bytes:0, sequence:0 };
        journal.record("input.text",json!({"bytes":12,"focus":"composer"})).unwrap();
        journal.record("test.oversize",json!({"metadata":"x".repeat(MAX_EVENT*2)})).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let rows: Vec<Value> = content.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(rows[0]["sequence"],1); assert_eq!(rows[1]["sequence"],2);
        assert_eq!(rows[0]["fields"]["bytes"],12); assert!(rows[1]["fields"]["omitted"].is_string());
        assert!(content.lines().all(|line| line.len() <= MAX_EVENT));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn native_debug_retention_preserves_live_writer_and_rotation() {
        let dir = std::env::temp_dir().join("opencode").join(format!("oc2-retention-{}-{}",std::process::id(),now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("daemon-live.jsonl");
        let mut journal = Journal { file:open(&path).unwrap(), path:path.clone(), bytes:0, sequence:0 };
        journal.record("before.prune",json!({})).unwrap();
        for index in 0..36 { std::fs::write(dir.join(format!("client-{index}.jsonl")),b"old\n").unwrap(); }
        prune(&dir).unwrap();
        assert!(path.exists(),"retention unlinked a live journal");
        journal.bytes = MAX_FILE;
        journal.record("after.prune",json!({})).unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().contains("after.prune"));
        assert!(path.with_extension("jsonl.1").exists());
        drop(journal);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn native_debug_deleted_path_recovers_accessible_logging() {
        let dir = std::env::temp_dir().join("opencode").join(format!("oc2-unlink-{}-{}",std::process::id(),now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("client-live.jsonl");
        let mut journal = Journal { file:open(&path).unwrap(), path:path.clone(), bytes:0, sequence:0 };
        std::fs::remove_file(&path).unwrap();
        journal.record("recovered",json!({})).unwrap();
        assert!(path.exists());
        assert!(std::fs::read_to_string(&path).unwrap().contains("recovered"));
        drop(journal);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
