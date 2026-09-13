//! Bounded structured-concurrency primitives for the native runtime.
#![forbid(unsafe_code)]
use std::{future::Future, sync::Arc, time::Duration};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{sync::{mpsc, Mutex, Semaphore}, task::JoinSet, time};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimeLimits { pub max_owned_tasks: usize, pub max_queue_items: usize, pub max_provider_calls: usize, pub max_blocking_jobs: usize, pub shutdown_grace_ms: u64 }
impl Default for RuntimeLimits { fn default() -> Self { Self { max_owned_tasks:256, max_queue_items:256, max_provider_calls:16, max_blocking_jobs:4, shutdown_grace_ms:5_000 } } }
impl RuntimeLimits { pub fn validate(&self) -> Result<(), RuntimeError> { for (name,value) in [("max_owned_tasks",self.max_owned_tasks),("max_queue_items",self.max_queue_items),("max_provider_calls",self.max_provider_calls),("max_blocking_jobs",self.max_blocking_jobs)] { if value == 0 { return Err(RuntimeError::InvalidLimit(name)); } } if self.shutdown_grace_ms == 0 { return Err(RuntimeError::InvalidLimit("shutdown_grace_ms")); } Ok(()) } }

#[derive(Debug, Error, Eq, PartialEq)] pub enum RuntimeError { #[error("runtime limit {0} must be non-zero")] InvalidLimit(&'static str), #[error("task scope is shutting down")] ShuttingDown, #[error("owned task limit reached")] TaskLimitReached, #[error("task shutdown exceeded grace period")] ShutdownTimedOut, #[error("owned task panicked or was cancelled: {0}")] Join(String) }

#[must_use = "task scopes must be shut down and joined by their owner"]
pub struct OwnedTaskScope { token: CancellationToken, permits: Arc<Semaphore>, tasks: Mutex<JoinSet<()>>, shutdown_grace: Duration }
impl OwnedTaskScope {
    pub fn new(limits:&RuntimeLimits)->Result<Self,RuntimeError>{ limits.validate()?; Ok(Self{token:CancellationToken::new(),permits:Arc::new(Semaphore::new(limits.max_owned_tasks)),tasks:Mutex::new(JoinSet::new()),shutdown_grace:Duration::from_millis(limits.shutdown_grace_ms)}) }
    #[must_use] pub fn cancellation_token(&self)->CancellationToken{self.token.child_token()}
    #[must_use] pub fn is_cancelled(&self)->bool{self.token.is_cancelled()}
    pub async fn spawn<F,Fut>(&self,task:F)->Result<(),RuntimeError> where F:FnOnce(CancellationToken)->Fut+Send+'static,Fut:Future<Output=()>+Send+'static { if self.token.is_cancelled(){return Err(RuntimeError::ShuttingDown);} let permit=Arc::clone(&self.permits).try_acquire_owned().map_err(|_|RuntimeError::TaskLimitReached)?; let token=self.token.child_token(); let mut tasks=self.tasks.lock().await; tasks.spawn(async move{let _permit=permit; task(token).await;}); Ok(()) }
    pub async fn shutdown(&self)->Result<(),RuntimeError>{ self.token.cancel(); let deadline=time::Instant::now()+self.shutdown_grace; let mut tasks=self.tasks.lock().await; loop { if tasks.is_empty(){return Ok(());} let remaining=deadline.saturating_duration_since(time::Instant::now()); if remaining.is_zero(){tasks.abort_all(); while tasks.join_next().await.is_some(){} return Err(RuntimeError::ShutdownTimedOut);} match time::timeout(remaining,tasks.join_next()).await { Ok(Some(Ok(())))=>{}, Ok(Some(Err(error)))=>return Err(RuntimeError::Join(error.to_string())), Ok(None)=>return Ok(()), Err(_)=>{tasks.abort_all(); while tasks.join_next().await.is_some(){} return Err(RuntimeError::ShutdownTimedOut);} } } }
}
impl Drop for OwnedTaskScope { fn drop(&mut self){self.token.cancel();} }
pub fn bounded_channel<T>(capacity:usize)->Result<(mpsc::Sender<T>,mpsc::Receiver<T>),RuntimeError>{ if capacity==0{return Err(RuntimeError::InvalidLimit("channel capacity"));} Ok(mpsc::channel(capacity)) }

#[cfg(test)] mod tests { use std::sync::{atomic::{AtomicBool,Ordering},Arc}; use super::*; #[tokio::test] async fn cancellation_reaches_owned_tasks(){let limits=RuntimeLimits{shutdown_grace_ms:1_000,..RuntimeLimits::default()}; let scope=OwnedTaskScope::new(&limits).unwrap(); let observed=Arc::new(AtomicBool::new(false)); let flag=Arc::clone(&observed); scope.spawn(move|token|async move{token.cancelled().await;flag.store(true,Ordering::SeqCst);}).await.unwrap(); scope.shutdown().await.unwrap(); assert!(observed.load(Ordering::SeqCst));} #[tokio::test] async fn scope_enforces_task_bound(){let limits=RuntimeLimits{max_owned_tasks:1,..RuntimeLimits::default()}; let scope=OwnedTaskScope::new(&limits).unwrap(); scope.spawn(|token|async move{token.cancelled().await}).await.unwrap(); assert_eq!(scope.spawn(|_|async{}).await,Err(RuntimeError::TaskLimitReached)); scope.shutdown().await.unwrap();} #[test] fn bounded_channel_rejects_zero(){assert!(bounded_channel::<u8>(0).is_err());} }
