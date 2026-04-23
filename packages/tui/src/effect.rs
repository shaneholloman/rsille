use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc::Sender, Arc};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub(crate) u64);

impl TaskId {
    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Queued,
    Running,
    RetryScheduled,
    Cancelling,
    Completed,
    Cancelled,
    TimedOut,
    Failed,
    Panicked,
}

impl TaskState {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            TaskState::Completed
                | TaskState::Cancelled
                | TaskState::TimedOut
                | TaskState::Failed
                | TaskState::Panicked
        )
    }
}

#[derive(Debug, Clone)]
pub struct TaskStatus {
    pub id: TaskId,
    pub key: Option<String>,
    pub label: Option<String>,
    pub state: TaskState,
    pub attempt: u32,
    pub max_attempts: u32,
    pub created_at: Instant,
    pub updated_at: Instant,
}

impl TaskStatus {
    pub(crate) fn new(
        id: TaskId,
        key: Option<String>,
        label: Option<String>,
        state: TaskState,
        attempt: u32,
        max_attempts: u32,
        created_at: Instant,
    ) -> Self {
        Self {
            id,
            key,
            label,
            state,
            attempt,
            max_attempts,
            created_at,
            updated_at: Instant::now(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskOutcome {
    #[default]
    Complete,
    Retry,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    max_retries: u32,
    delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::none()
    }
}

impl RetryPolicy {
    pub const fn none() -> Self {
        Self {
            max_retries: 0,
            delay: Duration::ZERO,
        }
    }

    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            delay: Duration::ZERO,
        }
    }

    pub fn fixed(max_retries: u32, delay: Duration) -> Self {
        Self { max_retries, delay }
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    pub fn max_retries(self) -> u32 {
        self.max_retries
    }

    pub fn delay(self) -> Duration {
        self.delay
    }

    pub fn max_attempts(self) -> u32 {
        self.max_retries.saturating_add(1)
    }

    pub(crate) fn can_retry(self, attempt: u32) -> bool {
        attempt <= self.max_retries
    }
}

#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

#[derive(Debug)]
pub enum Effect<M> {
    None,
    Message(M),
    Batch(Vec<Effect<M>>),
    Spawn(Task<M>),
    CancelTask(TaskId),
    CancelTaskKey(String),
    After(Duration, M),
}

impl<M> Effect<M> {
    pub fn none() -> Self {
        Self::None
    }

    pub fn message(message: M) -> Self {
        Self::Message(message)
    }

    pub fn batch(effects: impl IntoIterator<Item = Effect<M>>) -> Self {
        Self::Batch(effects.into_iter().collect())
    }

    pub fn spawn(task: Task<M>) -> Self {
        Self::Spawn(task)
    }

    pub fn cancel_task(task_id: TaskId) -> Self {
        Self::CancelTask(task_id)
    }

    pub fn cancel_task_key(key: impl Into<String>) -> Self {
        Self::CancelTaskKey(key.into())
    }

    pub fn after(duration: Duration, message: M) -> Self {
        Self::After(duration, message)
    }
}

type TaskRunner<M> = Arc<dyn Fn(TaskContext<M>) -> TaskOutcome + Send + Sync + 'static>;
pub(crate) type StatusHandler<M> = Arc<dyn Fn(TaskStatus) -> M + Send + Sync + 'static>;

pub struct Task<M> {
    pub(crate) key: Option<String>,
    pub(crate) label: Option<String>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) retry: RetryPolicy,
    pub(crate) runner: TaskRunner<M>,
    pub(crate) status_handler: Option<StatusHandler<M>>,
}

impl<M> Task<M> {
    pub fn new<F>(runner: F) -> Self
    where
        F: Fn(TaskContext<M>) -> TaskOutcome + Send + Sync + 'static,
    {
        Self {
            key: None,
            label: None,
            timeout: None,
            retry: RetryPolicy::none(),
            runner: Arc::new(runner),
            status_handler: None,
        }
    }

    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    pub fn on_status<F>(mut self, handler: F) -> Self
    where
        F: Fn(TaskStatus) -> M + Send + Sync + 'static,
    {
        self.status_handler = Some(Arc::new(handler));
        self
    }
}

impl<M> std::fmt::Debug for Task<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Task")
            .field("key", &self.key)
            .field("label", &self.label)
            .field("timeout", &self.timeout)
            .field("retry", &self.retry)
            .finish()
    }
}

pub(crate) struct TaskEventSender<M>(Sender<TaskEvent<M>>);

impl<M> Clone for TaskEventSender<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<M> TaskEventSender<M> {
    pub(crate) fn new(sender: Sender<TaskEvent<M>>) -> Self {
        Self(sender)
    }

    pub(crate) fn send(&self, event: TaskEvent<M>) {
        let _ = self.0.send(event);
    }
}

#[derive(Debug)]
pub(crate) enum TaskEvent<M> {
    Message(M),
    Status(TaskStatus),
}

pub struct TaskContext<M> {
    task_id: TaskId,
    attempt: u32,
    cancellation: CancellationToken,
    sender: TaskEventSender<M>,
}

impl<M> TaskContext<M> {
    pub(crate) fn new(
        task_id: TaskId,
        attempt: u32,
        cancellation: CancellationToken,
        sender: TaskEventSender<M>,
    ) -> Self {
        Self {
            task_id,
            attempt,
            cancellation,
            sender,
        }
    }

    pub fn task_id(&self) -> TaskId {
        self.task_id
    }

    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    pub fn emit(&self, message: M) {
        self.sender.send(TaskEvent::Message(message));
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    pub fn sleep(&self, duration: Duration) -> bool {
        sleep_with_cancellation(&self.cancellation, duration)
    }
}

pub struct UpdateCtx<'a, M> {
    effects: &'a mut Vec<Effect<M>>,
    task_statuses: Vec<TaskStatus>,
    task_keys: HashMap<String, TaskId>,
    now: Instant,
}

impl<'a, M> UpdateCtx<'a, M> {
    pub(crate) fn new(
        effects: &'a mut Vec<Effect<M>>,
        task_statuses: Vec<TaskStatus>,
        task_keys: HashMap<String, TaskId>,
        now: Instant,
    ) -> Self {
        Self {
            effects,
            task_statuses,
            task_keys,
            now,
        }
    }

    pub fn now(&self) -> Instant {
        self.now
    }

    pub fn emit(&mut self, message: M) {
        self.effects.push(Effect::message(message));
    }

    pub fn dispatch(&mut self, effect: Effect<M>) {
        self.effects.push(effect);
    }

    pub fn spawn(&mut self, task: Task<M>) {
        self.effects.push(Effect::spawn(task));
    }

    pub fn after(&mut self, duration: Duration, message: M) {
        self.effects.push(Effect::after(duration, message));
    }

    pub fn cancel_task(&mut self, task_id: TaskId) {
        self.effects.push(Effect::cancel_task(task_id));
    }

    pub fn cancel_task_key(&mut self, key: impl Into<String>) {
        self.effects.push(Effect::cancel_task_key(key));
    }

    pub fn tasks(&self) -> &[TaskStatus] {
        &self.task_statuses
    }

    pub fn task_status(&self, task_id: TaskId) -> Option<&TaskStatus> {
        self.task_statuses.iter().find(|status| status.id == task_id)
    }

    pub fn task_status_by_key(&self, key: &str) -> Option<&TaskStatus> {
        let task_id = self.task_keys.get(key)?;
        self.task_status(*task_id)
    }
}

pub(crate) fn run_task_attempt<M: Send + 'static>(
    task_id: TaskId,
    key: Option<String>,
    label: Option<String>,
    created_at: Instant,
    attempt: u32,
    retry: RetryPolicy,
    timeout: Option<Duration>,
    cancellation: CancellationToken,
    sender: TaskEventSender<M>,
    runner: TaskRunner<M>,
) -> TaskState {
    sender.send(TaskEvent::Status(TaskStatus::new(
        task_id,
        key.clone(),
        label.clone(),
        TaskState::Running,
        attempt,
        retry.max_attempts(),
        created_at,
    )));

    let timed_out = Arc::new(AtomicBool::new(false));
    let attempt_finished = Arc::new(AtomicBool::new(false));

    if let Some(timeout) = timeout {
        let cancellation = cancellation.clone();
        let timed_out = timed_out.clone();
        let attempt_finished = attempt_finished.clone();
        thread::spawn(move || {
            thread::sleep(timeout);
            if !attempt_finished.load(Ordering::SeqCst) {
                timed_out.store(true, Ordering::SeqCst);
                cancellation.cancel();
            }
        });
    }

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        runner(TaskContext::new(
            task_id,
            attempt,
            cancellation.clone(),
            sender.clone(),
        ))
    }));

    attempt_finished.store(true, Ordering::SeqCst);

    if timed_out.load(Ordering::SeqCst) {
        return TaskState::TimedOut;
    }

    if cancellation.is_cancelled() {
        return TaskState::Cancelled;
    }

    match outcome {
        Ok(TaskOutcome::Complete) => TaskState::Completed,
        Ok(TaskOutcome::Cancelled) => TaskState::Cancelled,
        Ok(TaskOutcome::Retry) if retry.can_retry(attempt) => TaskState::RetryScheduled,
        Ok(TaskOutcome::Retry) => TaskState::Failed,
        Err(_) => TaskState::Panicked,
    }
}

pub(crate) fn sleep_with_cancellation(
    cancellation: &CancellationToken,
    duration: Duration,
) -> bool {
    if duration.is_zero() {
        return !cancellation.is_cancelled();
    }

    let started_at = Instant::now();
    while started_at.elapsed() < duration {
        if cancellation.is_cancelled() {
            return false;
        }

        let remaining = duration.saturating_sub(started_at.elapsed());
        thread::sleep(remaining.min(Duration::from_millis(10)));
    }

    !cancellation.is_cancelled()
}

#[cfg(test)]
mod tests {
    use super::{CancellationToken, RetryPolicy, TaskState};
    use std::time::Duration;

    #[test]
    fn retry_policy_reports_attempt_budget() {
        let policy = RetryPolicy::fixed(2, Duration::from_millis(10));
        assert_eq!(policy.max_attempts(), 3);
        assert!(policy.can_retry(1));
        assert!(policy.can_retry(2));
        assert!(!policy.can_retry(3));
    }

    #[test]
    fn task_state_marks_terminal_variants() {
        assert!(TaskState::Completed.is_terminal());
        assert!(TaskState::Cancelled.is_terminal());
        assert!(!TaskState::Running.is_terminal());
    }

    #[test]
    fn cancellation_token_tracks_requests() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
    }
}
