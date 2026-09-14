//! Bounded structured-concurrency primitives for the native runtime.
#![forbid(unsafe_code)]
pub mod config;
pub mod effect;
pub mod lifecycle;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    future::Future,
    ops::Deref,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use thiserror::Error;
use tokio::{
    sync::{mpsc, Mutex, OnceCell, Semaphore},
    task::{JoinHandle, JoinSet},
    time,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimeLimits {
    pub max_owned_tasks: usize,
    pub max_queue_items: usize,
    pub max_provider_calls: usize,
    pub max_blocking_jobs: usize,
    pub shutdown_grace_ms: u64,
}
impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            max_owned_tasks: 256,
            max_queue_items: 256,
            max_provider_calls: 16,
            max_blocking_jobs: 4,
            shutdown_grace_ms: 5_000,
        }
    }
}
impl RuntimeLimits {
    pub fn validate(&self) -> Result<(), RuntimeError> {
        for (name, value) in [
            ("max_owned_tasks", self.max_owned_tasks),
            ("max_queue_items", self.max_queue_items),
            ("max_provider_calls", self.max_provider_calls),
            ("max_blocking_jobs", self.max_blocking_jobs),
        ] {
            if value == 0 {
                return Err(RuntimeError::InvalidLimit(name));
            }
        }
        if self.shutdown_grace_ms == 0 {
            return Err(RuntimeError::InvalidLimit("shutdown_grace_ms"));
        }
        Ok(())
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum RuntimeError {
    #[error("runtime limit {0} must be non-zero")]
    InvalidLimit(&'static str),
    #[error("task scope is shutting down")]
    ShuttingDown,
    #[error("owned task limit reached")]
    TaskLimitReached,
    #[error("task shutdown exceeded grace period")]
    ShutdownTimedOut,
    #[error("owned task panicked or was cancelled: {0}")]
    Join(String),
    #[error("byte budget exceeded: requested {requested}, available {available}")]
    BudgetExceeded { requested: usize, available: usize },
    #[error("service initialization failed: {0}")]
    ServiceInit(String),
    #[error("service concurrency limit reached")]
    ServiceLimitReached,
}

#[must_use = "task scopes must be shut down and joined by their owner"]
pub struct OwnedTaskScope {
    token: CancellationToken,
    permits: Arc<Semaphore>,
    tasks: Mutex<JoinSet<()>>,
    shutdown_grace: Duration,
}
impl OwnedTaskScope {
    pub fn new(limits: &RuntimeLimits) -> Result<Self, RuntimeError> {
        limits.validate()?;
        Ok(Self {
            token: CancellationToken::new(),
            permits: Arc::new(Semaphore::new(limits.max_owned_tasks)),
            tasks: Mutex::new(JoinSet::new()),
            shutdown_grace: Duration::from_millis(limits.shutdown_grace_ms),
        })
    }
    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.token.child_token()
    }
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
    pub async fn spawn<F, Fut>(&self, task: F) -> Result<(), RuntimeError>
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        if self.token.is_cancelled() {
            return Err(RuntimeError::ShuttingDown);
        }
        let permit = Arc::clone(&self.permits)
            .try_acquire_owned()
            .map_err(|_| RuntimeError::TaskLimitReached)?;
        let token = self.token.child_token();
        let mut tasks = self.tasks.lock().await;
        tasks.spawn(async move {
            let _permit = permit;
            task(token).await;
        });
        Ok(())
    }
    pub async fn shutdown(&self) -> Result<(), RuntimeError> {
        self.token.cancel();
        let deadline = time::Instant::now() + self.shutdown_grace;
        let mut tasks = self.tasks.lock().await;
        loop {
            if tasks.is_empty() {
                return Ok(());
            }
            let remaining = deadline.saturating_duration_since(time::Instant::now());
            if remaining.is_zero() {
                tasks.abort_all();
                while tasks.join_next().await.is_some() {}
                return Err(RuntimeError::ShutdownTimedOut);
            }
            match time::timeout(remaining, tasks.join_next()).await {
                Ok(Some(Ok(()))) => {}
                Ok(Some(Err(error))) => return Err(RuntimeError::Join(error.to_string())),
                Ok(None) => return Ok(()),
                Err(_) => {
                    tasks.abort_all();
                    while tasks.join_next().await.is_some() {}
                    return Err(RuntimeError::ShutdownTimedOut);
                }
            }
        }
    }
}
impl Drop for OwnedTaskScope {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
pub fn bounded_channel<T>(
    capacity: usize,
) -> Result<(mpsc::Sender<T>, mpsc::Receiver<T>), RuntimeError> {
    if capacity == 0 {
        return Err(RuntimeError::InvalidLimit("channel capacity"));
    }
    Ok(mpsc::channel(capacity))
}

#[derive(Clone, Debug)]
pub struct ByteBudget {
    limit: usize,
    used: Arc<AtomicUsize>,
}
impl ByteBudget {
    pub fn new(limit: usize) -> Result<Self, RuntimeError> {
        if limit == 0 {
            return Err(RuntimeError::InvalidLimit("byte_budget"));
        }
        Ok(Self {
            limit,
            used: Arc::new(AtomicUsize::new(0)),
        })
    }
    pub fn acquire(&self, bytes: usize) -> Result<(), RuntimeError> {
        let mut current = self.used.load(Ordering::SeqCst);
        loop {
            let available = self.limit.saturating_sub(current);
            if current
                .checked_add(bytes)
                .map_or(true, |next| next > self.limit)
            {
                return Err(RuntimeError::BudgetExceeded {
                    requested: bytes,
                    available,
                });
            }
            match self.used.compare_exchange_weak(
                current,
                current + bytes,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return Ok(()),
                Err(actual) => current = actual,
            }
        }
    }
    pub fn release(&self, bytes: usize) {
        let mut current = self.used.load(Ordering::SeqCst);
        loop {
            let next = current.saturating_sub(bytes);
            match self
                .used
                .compare_exchange_weak(current, next, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return,
                Err(actual) => current = actual,
            }
        }
    }
    pub fn reserve(&self, bytes: usize) -> Result<ByteBudgetReservation, RuntimeError> {
        self.acquire(bytes)?;
        Ok(ByteBudgetReservation {
            budget: self.clone(),
            bytes,
        })
    }
    #[must_use]
    pub fn used(&self) -> usize {
        self.used.load(Ordering::SeqCst)
    }
    #[must_use]
    pub fn limit(&self) -> usize {
        self.limit
    }
    #[must_use]
    pub fn available(&self) -> usize {
        self.limit.saturating_sub(self.used.load(Ordering::SeqCst))
    }
    pub fn try_consume(&self, bytes: usize) -> Result<(), RuntimeError> {
        self.acquire(bytes)
    }
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.available()
    }
    pub fn reset(&self) {
        self.release(self.used())
    }
}

#[must_use = "reservations auto-release on drop unless leaked"]
#[derive(Debug)]
pub struct ByteBudgetReservation {
    budget: ByteBudget,
    bytes: usize,
}
impl ByteBudgetReservation {
    pub fn leak(mut self) -> usize {
        let b = self.bytes;
        self.bytes = 0;
        b
    }
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }
}
impl Drop for ByteBudgetReservation {
    fn drop(&mut self) {
        if self.bytes > 0 {
            self.budget.release(self.bytes);
        }
    }
}

type AsyncInit<T> =
    Box<dyn Fn() -> Pin<Box<dyn Future<Output = Result<T, RuntimeError>> + Send>> + Send + Sync>;

pub struct LazyService<T> {
    cell: OnceCell<Arc<T>>,
    init: Mutex<Option<AsyncInit<T>>>,
    permits: Option<Arc<Semaphore>>,
}
impl<T: fmt::Debug> fmt::Debug for LazyService<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LazyService")
            .field("initialized", &self.cell.initialized())
            .field("service", &self.cell.get())
            .finish()
    }
}
pub struct LazyServicePermit<T> {
    service: Arc<T>,
    _permit: Option<tokio::sync::OwnedSemaphorePermit>,
}
impl<T: fmt::Debug> fmt::Debug for LazyServicePermit<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LazyServicePermit")
            .field("service", &self.service)
            .finish()
    }
}
impl<T> Deref for LazyServicePermit<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.service
    }
}

impl<T: Send + Sync + 'static> LazyService<T> {
    pub fn new<F, Fut>(init: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, RuntimeError>> + Send + 'static,
    {
        Self {
            cell: OnceCell::new(),
            init: Mutex::new(Some(Box::new(move || Box::pin(init())))),
            permits: None,
        }
    }
    pub fn bounded<F, Fut>(max_concurrency: usize, init: F) -> Result<Self, RuntimeError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, RuntimeError>> + Send + 'static,
    {
        if max_concurrency == 0 {
            return Err(RuntimeError::InvalidLimit("lazy_service_concurrency"));
        }
        Ok(Self {
            cell: OnceCell::new(),
            init: Mutex::new(Some(Box::new(move || Box::pin(init())))),
            permits: Some(Arc::new(Semaphore::new(max_concurrency))),
        })
    }
    pub async fn get(&self) -> Result<Arc<T>, RuntimeError> {
        self.cell
            .get_or_try_init(|| async {
                let init_fn = {
                    let guard = self.init.lock().await;
                    guard.as_ref().map(|f| f()).ok_or_else(|| {
                        RuntimeError::ServiceInit("initializer unavailable".to_string())
                    })?
                };
                let service = init_fn.await?;
                Ok(Arc::new(service))
            })
            .await
            .cloned()
    }
    pub async fn acquire(&self) -> Result<LazyServicePermit<T>, RuntimeError> {
        let service = self.get().await?;
        let permit = match &self.permits {
            Some(sem) => Some(
                Arc::clone(sem)
                    .acquire_owned()
                    .await
                    .map_err(|_| RuntimeError::ShuttingDown)?,
            ),
            None => None,
        };
        Ok(LazyServicePermit {
            service,
            _permit: permit,
        })
    }
    pub fn try_acquire(&self) -> Result<LazyServicePermit<T>, RuntimeError> {
        let service = self
            .cell
            .get()
            .cloned()
            .ok_or_else(|| RuntimeError::ServiceInit("service not initialized".to_string()))?;
        let permit = match &self.permits {
            Some(sem) => Some(
                Arc::clone(sem)
                    .try_acquire_owned()
                    .map_err(|_| RuntimeError::ServiceLimitReached)?,
            ),
            None => None,
        };
        Ok(LazyServicePermit {
            service,
            _permit: permit,
        })
    }
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.cell.initialized()
    }
}

#[derive(Debug)]
pub struct HeartbeatGuard {
    token: CancellationToken,
    handle: Option<JoinHandle<()>>,
    ticks: Arc<AtomicU64>,
}
impl HeartbeatGuard {
    pub fn spawn<F, Fut>(interval: Duration, mut action: F) -> Result<Self, RuntimeError>
    where
        F: FnMut(u64) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        if interval.is_zero() {
            return Err(RuntimeError::InvalidLimit("heartbeat_interval"));
        }
        let token = CancellationToken::new();
        let child_token = token.child_token();
        let ticks = Arc::new(AtomicU64::new(0));
        let ticks_clone = Arc::clone(&ticks);
        let handle = tokio::spawn(async move {
            let mut timer = time::interval(interval);
            timer.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
            timer.tick().await;
            loop {
                tokio::select! {
                    _ = child_token.cancelled() => break,
                    _ = timer.tick() => {
                        if child_token.is_cancelled() { break; }
                        let count = ticks_clone.fetch_add(1, Ordering::SeqCst) + 1;
                        action(count).await;
                    }
                }
            }
        });
        Ok(Self {
            token,
            handle: Some(handle),
            ticks,
        })
    }
    pub fn spawn_with_token<F, Fut>(
        parent: &CancellationToken,
        interval: Duration,
        mut action: F,
    ) -> Result<Self, RuntimeError>
    where
        F: FnMut(u64) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        if interval.is_zero() {
            return Err(RuntimeError::InvalidLimit("heartbeat_interval"));
        }
        let token = parent.child_token();
        let child_token = token.child_token();
        let ticks = Arc::new(AtomicU64::new(0));
        let ticks_clone = Arc::clone(&ticks);
        let handle = tokio::spawn(async move {
            let mut timer = time::interval(interval);
            timer.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
            timer.tick().await;
            loop {
                tokio::select! {
                    _ = child_token.cancelled() => break,
                    _ = timer.tick() => {
                        if child_token.is_cancelled() { break; }
                        let count = ticks_clone.fetch_add(1, Ordering::SeqCst) + 1;
                        action(count).await;
                    }
                }
            }
        });
        Ok(Self {
            token,
            handle: Some(handle),
            ticks,
        })
    }
    #[must_use]
    pub fn ticks(&self) -> u64 {
        self.ticks.load(Ordering::SeqCst)
    }
    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.token.child_token()
    }
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
    pub fn cancel(&self) {
        self.token.cancel();
    }
}
impl Drop for HeartbeatGuard {
    fn drop(&mut self) {
        self.token.cancel();
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };

    #[tokio::test]
    async fn cancellation_reaches_owned_tasks() {
        let limits = RuntimeLimits {
            shutdown_grace_ms: 1_000,
            ..RuntimeLimits::default()
        };
        let scope = OwnedTaskScope::new(&limits).unwrap();
        let observed = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&observed);
        scope
            .spawn(move |token| async move {
                token.cancelled().await;
                flag.store(true, Ordering::SeqCst);
            })
            .await
            .unwrap();
        scope.shutdown().await.unwrap();
        assert!(observed.load(Ordering::SeqCst));
    }
    #[tokio::test]
    async fn scope_enforces_task_bound() {
        let limits = RuntimeLimits {
            max_owned_tasks: 1,
            ..RuntimeLimits::default()
        };
        let scope = OwnedTaskScope::new(&limits).unwrap();
        scope
            .spawn(|token| async move { token.cancelled().await })
            .await
            .unwrap();
        assert_eq!(
            scope.spawn(|_| async {}).await,
            Err(RuntimeError::TaskLimitReached)
        );
        scope.shutdown().await.unwrap();
    }
    #[test]
    fn bounded_channel_rejects_zero() {
        assert!(bounded_channel::<u8>(0).is_err());
    }

    #[test]
    fn byte_budget_enforces_limit() {
        let budget = ByteBudget::new(100).unwrap();
        assert_eq!(budget.limit(), 100);
        assert_eq!(budget.available(), 100);
        assert!(budget.acquire(60).is_ok());
        assert_eq!(budget.used(), 60);
        assert_eq!(budget.available(), 40);
        assert_eq!(
            budget.acquire(50),
            Err(RuntimeError::BudgetExceeded {
                requested: 50,
                available: 40
            })
        );
        budget.release(30);
        assert_eq!(budget.used(), 30);
        assert!(budget.acquire(50).is_ok());
        assert_eq!(budget.used(), 80);
    }

    #[test]
    fn byte_budget_reservation_auto_releases() {
        let budget = ByteBudget::new(100).unwrap();
        {
            let res = budget.reserve(40).unwrap();
            assert_eq!(res.bytes(), 40);
            assert_eq!(budget.used(), 40);
        }
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn byte_budget_rejects_zero_limit() {
        assert_eq!(
            ByteBudget::new(0).unwrap_err(),
            RuntimeError::InvalidLimit("byte_budget")
        );
    }

    #[tokio::test]
    async fn lazy_service_constructs_on_first_use() {
        let init_count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&init_count);
        let service = LazyService::new(move || {
            let c = Arc::clone(&count_clone);
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok("initialized_service".to_string())
            }
        });

        assert!(!service.is_initialized());
        assert_eq!(init_count.load(Ordering::SeqCst), 0);

        let val1 = service.get().await.unwrap();
        assert_eq!(*val1, "initialized_service");
        assert!(service.is_initialized());
        assert_eq!(init_count.load(Ordering::SeqCst), 1);

        let val2 = service.get().await.unwrap();
        assert_eq!(*val2, "initialized_service");
        assert_eq!(init_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn lazy_service_bounded_concurrency() {
        let service = LazyService::bounded(1, || async { Ok(42) }).unwrap();
        let permit1 = service.acquire().await.unwrap();
        assert_eq!(*permit1, 42);

        assert_eq!(
            service.try_acquire().err(),
            Some(RuntimeError::ServiceLimitReached)
        );
        drop(permit1);

        let permit2 = service.try_acquire().unwrap();
        assert_eq!(*permit2, 42);
    }

    #[tokio::test]
    async fn heartbeat_guard_ticks_and_cancels_on_drop() {
        let ticks = Arc::new(AtomicU64::new(0));
        let ticks_clone = Arc::clone(&ticks);
        let guard = HeartbeatGuard::spawn(Duration::from_millis(10), move |_| {
            let t = Arc::clone(&ticks_clone);
            async move {
                t.fetch_add(1, Ordering::SeqCst);
            }
        })
        .unwrap();

        time::sleep(Duration::from_millis(35)).await;
        let count_before = guard.ticks();
        assert!(count_before >= 2);
        assert!(!guard.is_cancelled());

        drop(guard);
        time::sleep(Duration::from_millis(25)).await;
        let count_after = ticks.load(Ordering::SeqCst);
        time::sleep(Duration::from_millis(25)).await;
        assert_eq!(ticks.load(Ordering::SeqCst), count_after);
    }

    #[test]
    fn heartbeat_guard_rejects_zero_interval() {
        assert_eq!(
            HeartbeatGuard::spawn(Duration::ZERO, |_| async {}).unwrap_err(),
            RuntimeError::InvalidLimit("heartbeat_interval")
        );
    }

    #[test]
    fn byte_budget_aliases_try_consume_remaining_reset() {
        let b = ByteBudget::new(10).unwrap();
        b.try_consume(4).unwrap();
        assert_eq!(b.remaining(), 6);
        assert_eq!(
            b.try_consume(7),
            Err(RuntimeError::BudgetExceeded {
                requested: 7,
                available: 6
            })
        );
        b.reset();
        assert_eq!(b.used(), 0);
        assert_eq!(b.remaining(), 10);
    }
}
