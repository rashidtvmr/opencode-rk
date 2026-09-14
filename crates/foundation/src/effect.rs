//! Async effect execution with retry, timeout, and result capture (BASE-008).
//!
//! Effects are units of work (DB query, file op, HTTP call) with uniform
//! outcome handling. Only `Io` executes today; `Db`/`Net` validate via
//! `dry_run` but `run` returns `Failure("not implemented")`.
//! Retry: any `Failure` is retried up to `max_retries` times (total
//! `1 + max_retries` attempts). `Timeout` is terminal, never retried.
//! `default_timeout` bounds each attempt. `run_op` exposes the same
//! retry/timeout driver for custom ops (tests use it for slow/transient
//! behavior that real file ops cannot produce).
use std::{path::PathBuf, time::{Duration, Instant}};
use tokio::{task::JoinSet, time::timeout};
#[derive(Clone, Debug, Eq, PartialEq)] pub enum Effect { Db(DbEffect), Io(IoEffect), Net(NetEffect) }
#[derive(Clone, Debug, Eq, PartialEq)] pub enum DbEffect { Query(String), Execute(String) }
#[derive(Clone, Debug, Eq, PartialEq)] pub enum IoEffect { ReadFile(PathBuf), WriteFile(PathBuf, Vec<u8>) }
#[derive(Clone, Debug, Eq, PartialEq)] pub enum NetEffect { HttpGet(String), HttpPost(String, Vec<u8>) }
#[derive(Clone, Debug, Eq, PartialEq)] pub enum Outcome { Success(Vec<u8>), Failure(String), Timeout }
#[derive(Clone, Debug, Eq, PartialEq)] pub struct EffectResult { pub effect: Effect, pub outcome: Outcome, pub duration: Duration }
#[derive(Clone, Copy, Debug)] pub struct EffectRunner { default_timeout: Duration, max_retries: u32 }
fn valid_url(url: &str) -> Result<(), String> { if (url.starts_with("http://") && url.len() > 7) || (url.starts_with("https://") && url.len() > 8) { Ok(()) } else { Err(format!("net url must start with http:// or https://: {url}")) } }
impl EffectRunner {
    #[must_use] pub fn new(default_timeout: Duration, max_retries: u32) -> Self { Self { default_timeout, max_retries } }
    #[must_use] pub fn default_timeout(&self) -> Duration { self.default_timeout }
    #[must_use] pub fn max_retries(&self) -> u32 { self.max_retries }
    async fn execute_once(&self, effect: &Effect) -> Outcome { match effect { Effect::Io(IoEffect::ReadFile(p)) => tokio::fs::read(p).await.map(Outcome::Success).unwrap_or_else(|e| Outcome::Failure(e.to_string())), Effect::Io(IoEffect::WriteFile(p, data)) => tokio::fs::write(p, data).await.map(|()| Outcome::Success(Vec::new())).unwrap_or_else(|e| Outcome::Failure(e.to_string())), Effect::Db(_) | Effect::Net(_) => Outcome::Failure("not implemented".to_string()) } }
    pub async fn run_op<F, Fut>(&self, mut attempt: F) -> (Outcome, Duration) where F: FnMut(u32) -> Fut + Send, Fut: std::future::Future<Output = Outcome> + Send { let start = Instant::now(); let mut n = 0u32; loop { match timeout(self.default_timeout, attempt(n)).await { Ok(Outcome::Success(b)) => return (Outcome::Success(b), start.elapsed()), Ok(Outcome::Timeout) | Err(_) => return (Outcome::Timeout, start.elapsed()), Ok(f @ Outcome::Failure(_)) => { if n >= self.max_retries { return (f, start.elapsed()); } n += 1; } } } }
    pub async fn run(&self, effect: Effect) -> EffectResult { let (outcome, duration) = self.run_op(|_| self.execute_once(&effect)).await; EffectResult { effect, outcome, duration } }
    pub async fn run_batch(&self, effects: Vec<Effect>) -> Vec<EffectResult> { let mut set = JoinSet::new(); for (i, e) in effects.into_iter().enumerate() { let r = *self; set.spawn(async move { (i, r.run(e).await) }); } let mut slots: Vec<Option<EffectResult>> = Vec::new(); while let Some(joined) = set.join_next().await { if let Ok((i, res)) = joined { while slots.len() <= i { slots.push(None); } slots[i] = Some(res); } } slots.into_iter().flatten().collect() }
    pub fn dry_run(&self, effect: &Effect) -> Result<(), String> { match effect { Effect::Db(DbEffect::Query(q)) | Effect::Db(DbEffect::Execute(q)) => if q.trim().is_empty() { Err("db statement must be non-empty".to_string()) } else { Ok(()) }, Effect::Io(IoEffect::ReadFile(p)) | Effect::Io(IoEffect::WriteFile(p, _)) => if p.as_os_str().is_empty() { Err("io path must be non-empty".to_string()) } else { Ok(()) }, Effect::Net(NetEffect::HttpGet(u)) | Effect::Net(NetEffect::HttpPost(u, _)) => valid_url(u) } }
}
#[cfg(test)] mod tests {
    use super::*; use std::sync::{atomic::{AtomicU32, AtomicU64, Ordering}, Arc};
    static TMP_N: AtomicU64 = AtomicU64::new(0);
    fn tmp_path(tag: &str) -> PathBuf { std::env::temp_dir().join(format!("opencode-rk-effect-{}-{}-{}.bin", std::process::id(), TMP_N.fetch_add(1, Ordering::SeqCst), tag)) }
    fn cleanup(p: &PathBuf) { let _ = std::fs::remove_file(p); }
    #[tokio::test] async fn io_read_write_roundtrip() { let r = EffectRunner::new(Duration::from_secs(5), 1); let p = tmp_path("roundtrip"); let data = b"effect-rt-hello".to_vec(); let w = r.run(Effect::Io(IoEffect::WriteFile(p.clone(), data.clone()))).await; assert_eq!(w.effect, Effect::Io(IoEffect::WriteFile(p.clone(), data.clone()))); assert_eq!(w.outcome, Outcome::Success(Vec::new())); let rd = r.run(Effect::Io(IoEffect::ReadFile(p.clone()))).await; assert_eq!(rd.effect, Effect::Io(IoEffect::ReadFile(p.clone()))); assert_eq!(rd.outcome, Outcome::Success(data)); cleanup(&p); }
    #[tokio::test] async fn timeout_returns_timeout_outcome() { let r = EffectRunner::new(Duration::from_millis(100), 3); let tries = Arc::new(AtomicU32::new(0)); let c = Arc::clone(&tries); let (o, d) = r.run_op(|_| { let c = Arc::clone(&c); async move { c.fetch_add(1, Ordering::SeqCst); tokio::time::sleep(Duration::from_secs(30)).await; Outcome::Success(vec![]) } }).await; assert_eq!(o, Outcome::Timeout); assert_eq!(tries.load(Ordering::SeqCst), 1); assert!(d < Duration::from_secs(5)); drop(c); }
    #[test] fn dry_run_validates() { let r = EffectRunner::new(Duration::from_secs(1), 0); assert!(r.dry_run(&Effect::Io(IoEffect::WriteFile(tmp_path("dry"), vec![1]))).is_ok()); assert!(r.dry_run(&Effect::Io(IoEffect::ReadFile(PathBuf::from("")))).is_err()); assert!(r.dry_run(&Effect::Net(NetEffect::HttpGet("https://example.com".to_string()))).is_ok()); assert!(r.dry_run(&Effect::Net(NetEffect::HttpGet("ftp://x".to_string()))).is_err()); assert!(r.dry_run(&Effect::Db(DbEffect::Query("SELECT 1".to_string()))).is_ok()); assert!(r.dry_run(&Effect::Db(DbEffect::Query("  ".to_string()))).is_err()); }
    #[tokio::test] async fn batch_parallel() { let r = EffectRunner::new(Duration::from_secs(5), 0); let paths: Vec<PathBuf> = (0..3).map(|i| tmp_path(&format!("batch{i}"))).collect(); let effects: Vec<Effect> = paths.iter().enumerate().map(|(i, p)| Effect::Io(IoEffect::WriteFile(p.clone(), vec![i as u8; 8]))).collect(); let results = r.run_batch(effects).await; assert_eq!(results.len(), 3); for (i, res) in results.iter().enumerate() { assert_eq!(res.outcome, Outcome::Success(Vec::new())); assert_eq!(std::fs::read(&paths[i]).unwrap(), vec![i as u8; 8]); cleanup(&paths[i]); } }
    #[tokio::test] async fn retry_on_transient() { let r = EffectRunner::new(Duration::from_secs(5), 5); let tries = Arc::new(AtomicU32::new(0)); let c = Arc::clone(&tries); let (o, _) = r.run_op(move |_| { let c = Arc::clone(&c); async move { let n = c.fetch_add(1, Ordering::SeqCst) + 1; if n < 3 { Outcome::Failure("transient".to_string()) } else { Outcome::Success(vec![7]) } } }).await; assert_eq!(o, Outcome::Success(vec![7])); assert_eq!(tries.load(Ordering::SeqCst), 3); }
}
