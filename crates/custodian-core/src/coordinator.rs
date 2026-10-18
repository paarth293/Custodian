//! Task coordinator with an optional sealed lifecycle journal (Milestone M6 / F46).
//!
//! Separates long-running work from chat turns, enforces honest task state
//! transitions, prevents concurrent mutation races on the same resources,
//! and provides independent per-task cancellation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;

/// Honest task lifecycle states (§10.2 / F46).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskState {
    Queued,
    Inspecting,
    NeedsClarification,
    WaitingApproval,
    Preparing,
    Running,
    Verifying,
    Completed,
    Partial,
    Cancelled,
    Failed,
    RecoveryRequired,
}

impl TaskState {
    /// True if the state represents an ended task that will do no further forward work.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskState::Completed
                | TaskState::Partial
                | TaskState::Cancelled
                | TaskState::Failed
                | TaskState::RecoveryRequired
        )
    }

    /// True if the task is actively scheduled or executing.
    pub fn is_active(&self) -> bool {
        !self.is_terminal()
    }

    /// Validates whether a state transition is allowed by the lifecycle state machine.
    pub fn can_transition_to(&self, next: TaskState) -> bool {
        if *self == next {
            return true;
        }
        if self.is_terminal() {
            return false;
        }
        // Any active task can transition to Cancelled, Failed, or RecoveryRequired.
        if matches!(
            next,
            TaskState::Cancelled | TaskState::Failed | TaskState::RecoveryRequired
        ) {
            return true;
        }
        match self {
            TaskState::Queued => matches!(
                next,
                TaskState::Inspecting
                    | TaskState::Preparing
                    | TaskState::Running
                    | TaskState::WaitingApproval
            ),
            TaskState::Inspecting => matches!(
                next,
                TaskState::NeedsClarification
                    | TaskState::WaitingApproval
                    | TaskState::Preparing
                    | TaskState::Running
                    | TaskState::Partial
                    | TaskState::Completed
            ),
            TaskState::NeedsClarification => matches!(
                next,
                TaskState::Inspecting | TaskState::WaitingApproval | TaskState::Running
            ),
            TaskState::WaitingApproval => {
                matches!(next, TaskState::Preparing | TaskState::Running)
            }
            TaskState::Preparing => matches!(next, TaskState::Running),
            TaskState::Running => {
                matches!(
                    next,
                    TaskState::Verifying | TaskState::Partial | TaskState::Completed
                )
            }
            TaskState::Verifying => {
                matches!(next, TaskState::Completed | TaskState::Partial)
            }
            _ => false,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TaskError {
    #[error("Task journal could not be saved or read: {0}")]
    Persistence(String),
    #[error("Task {0} not found")]
    NotFound(String),
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidTransition { from: TaskState, to: TaskState },
    #[error("Resource conflict: path {path} is already locked by active task {holder}")]
    ResourceConflict { path: String, holder: String },
}

/// Public representation of a task for queries and reporting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskInfo {
    pub id: String,
    pub description: String,
    pub state: TaskState,
    pub progress: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub resources: Vec<String>,
    #[serde(default)]
    pub execution: Option<crate::task_program::Checkpoint>,
}

#[derive(Debug)]
struct ManagedTask {
    info: TaskInfo,
    cancel_flag: Arc<AtomicBool>,
    locked_paths: Vec<PathBuf>,
    pause_requested: bool,
}

/// Coordinates tasks, lifecycle transitions, per-task cancellation, and resource locks.
#[derive(Default)]
pub struct TaskCoordinator {
    tasks: Mutex<HashMap<String, ManagedTask>>,
    // Maps locked canonical/normalized path to the TaskId holding it.
    path_locks: Mutex<HashMap<PathBuf, String>>,
    journal: Option<PathBuf>,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn normalize_path(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut lexical = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                lexical.pop();
            }
            other => lexical.push(other.as_os_str()),
        }
    }
    // Resolve an existing ancestor even when the destination does not exist yet.
    let mut ancestor = lexical.as_path();
    let mut suffix = Vec::new();
    let resolved = loop {
        if let Ok(mut resolved) = std::fs::canonicalize(ancestor) {
            for name in suffix.iter().rev() {
                resolved.push(name);
            }
            break resolved;
        }
        let (Some(name), Some(parent)) = (ancestor.file_name(), ancestor.parent()) else {
            break lexical;
        };
        suffix.push(name.to_os_string());
        ancestor = parent;
    };
    #[cfg(windows)]
    {
        PathBuf::from(resolved.to_string_lossy().to_lowercase())
    }
    #[cfg(not(windows))]
    {
        resolved
    }
}

impl TaskCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reopens lifecycle history. Interrupted work is never replayed automatically.
    pub fn open(journal: PathBuf) -> Result<Self, TaskError> {
        let mut saved: Vec<TaskInfo> = match std::fs::read(&journal) {
            Ok(sealed) => {
                let plain = custodian_winsec::dpapi::unprotect(&sealed)
                    .map_err(|e| TaskError::Persistence(e.to_string()))?;
                serde_json::from_slice(&plain).map_err(|e| TaskError::Persistence(e.to_string()))?
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(err) => return Err(TaskError::Persistence(err.to_string())),
        };
        let mut tasks = HashMap::new();
        for mut info in saved.drain(..) {
            if info.state.is_active() {
                info.state = TaskState::RecoveryRequired;
                info.progress = Some("Interrupted when Custodian stopped. Inspect the files before retrying; nothing was replayed.".into());
                info.updated_at = now_unix();
                if let Some(execution) = &mut info.execution {
                    execution.paused = false;
                    execution.pause_requested = false;
                }
            }
            let id = info.id.clone();
            if tasks
                .insert(
                    id,
                    ManagedTask {
                        info,
                        cancel_flag: Arc::new(AtomicBool::new(true)),
                        locked_paths: Vec::new(),
                        pause_requested: false,
                    },
                )
                .is_some()
            {
                return Err(TaskError::Persistence(
                    "duplicate task identity in journal".into(),
                ));
            }
        }
        let coordinator = Self {
            tasks: Mutex::new(tasks),
            path_locks: Mutex::new(HashMap::new()),
            journal: Some(journal),
        };
        coordinator.persist(&coordinator.tasks.lock().unwrap_or_else(|e| e.into_inner()))?;
        Ok(coordinator)
    }

    fn persist(&self, tasks: &HashMap<String, ManagedTask>) -> Result<(), TaskError> {
        let Some(path) = &self.journal else {
            return Ok(());
        };
        let fail = |e: &dyn std::fmt::Display| TaskError::Persistence(e.to_string());
        let mut infos: Vec<_> = tasks.values().map(|task| &task.info).collect();
        infos.sort_by(|a, b| a.id.cmp(&b.id));
        let plain = serde_json::to_vec(&infos).map_err(|e| fail(&e))?;
        let sealed = custodian_winsec::dpapi::protect(&plain).map_err(|e| fail(&e))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| fail(&e))?;
        }
        let temporary = path.with_extension("tmp");
        let mut file = std::fs::File::create(&temporary).map_err(|e| fail(&e))?;
        file.write_all(&sealed).map_err(|e| fail(&e))?;
        file.sync_all().map_err(|e| fail(&e))?;
        drop(file);
        std::fs::rename(&temporary, path).map_err(|e| fail(&e))?;
        Ok(())
    }

    /// Registers and starts a new managed task with resource locks.
    /// Returns `(task_id, cancel_flag)` or `TaskError::ResourceConflict` if another task holds locks.
    pub fn create_task(
        &self,
        description: String,
        initial_state: TaskState,
        resources: &[PathBuf],
    ) -> Result<(String, Arc<AtomicBool>), TaskError> {
        let normalized: Vec<PathBuf> = resources.iter().map(|p| normalize_path(p)).collect();
        let task_id = Uuid::new_v4().to_string();

        // Always acquire tasks before path_locks, matching completion/cancellation.
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        let mut locks = self.path_locks.lock().unwrap_or_else(|e| e.into_inner());

        // Check for conflicting locks
        for path in &normalized {
            if let Some((_, holder)) = locks
                .iter()
                .find(|(locked, _)| path.starts_with(locked) || locked.starts_with(path))
            {
                return Err(TaskError::ResourceConflict {
                    path: path.display().to_string(),
                    holder: holder.clone(),
                });
            }
        }

        // Acquire locks
        if initial_state.is_active() {
            for path in &normalized {
                locks.insert(path.clone(), task_id.clone());
            }
        }

        let now = now_unix();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let task = ManagedTask {
            info: TaskInfo {
                id: task_id.clone(),
                description,
                state: initial_state,
                progress: None,
                created_at: now,
                updated_at: now,
                resources: resources.iter().map(|p| p.display().to_string()).collect(),
                execution: None,
            },
            cancel_flag: Arc::clone(&cancel_flag),
            locked_paths: normalized,
            pause_requested: false,
        };

        tasks.insert(task_id.clone(), task);
        if let Err(err) = self.persist(&tasks) {
            tasks.remove(&task_id);
            locks.retain(|_, holder| holder != &task_id);
            return Err(err);
        }

        Ok((task_id, cancel_flag))
    }

    /// Reserve measured input/output resources before the first program effect.
    pub fn reserve_resources(&self, id: &str, resources: &[PathBuf]) -> Result<(), TaskError> {
        let normalized: Vec<_> = resources.iter().map(|p| normalize_path(p)).collect();
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        let task = tasks
            .get_mut(id)
            .ok_or_else(|| TaskError::NotFound(id.into()))?;
        if task.info.state != TaskState::Running || task.cancel_flag.load(Ordering::Acquire) {
            return Err(TaskError::InvalidTransition {
                from: task.info.state,
                to: TaskState::Running,
            });
        }
        let mut locks = self.path_locks.lock().unwrap_or_else(|e| e.into_inner());
        for path in &normalized {
            if let Some((_, holder)) = locks.iter().find(|(locked, holder)| {
                holder.as_str() != id && (path.starts_with(locked) || locked.starts_with(path))
            }) {
                return Err(TaskError::ResourceConflict {
                    path: path.display().to_string(),
                    holder: holder.clone(),
                });
            }
        }
        let previous = task.info.clone();
        let old_paths = task.locked_paths.clone();
        for (path, original) in normalized.into_iter().zip(resources) {
            locks.insert(path.clone(), id.into());
            if !task.locked_paths.contains(&path) {
                task.locked_paths.push(path);
                task.info.resources.push(original.display().to_string());
            }
        }
        if let Err(error) = self.persist(&tasks) {
            let task = tasks.get_mut(id).unwrap();
            task.info = previous;
            task.locked_paths = old_paths;
            locks.retain(|_, holder| holder != id);
            for path in &task.locked_paths {
                locks.insert(path.clone(), id.into());
            }
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn start_program(
        &self,
        id: &str,
        checkpoint: &crate::task_program::Checkpoint,
    ) -> Result<(), TaskError> {
        self.store_program_checkpoint(id, checkpoint, true)
    }
    pub(crate) fn program_checkpoint(
        &self,
        id: &str,
        checkpoint: &crate::task_program::Checkpoint,
    ) -> Result<(), TaskError> {
        self.store_program_checkpoint(id, checkpoint, false)
    }
    fn store_program_checkpoint(
        &self,
        id: &str,
        checkpoint: &crate::task_program::Checkpoint,
        initial: bool,
    ) -> Result<(), TaskError> {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        let task = tasks
            .get_mut(id)
            .ok_or_else(|| TaskError::NotFound(id.into()))?;
        if task.info.state != TaskState::Running || (initial && task.info.execution.is_some()) {
            return Err(TaskError::InvalidTransition {
                from: task.info.state,
                to: TaskState::Running,
            });
        }
        let previous = task.info.clone();
        let mut checkpoint = checkpoint.clone();
        checkpoint.pause_requested = task.pause_requested;
        task.info.execution = Some(checkpoint);
        task.info.updated_at = now_unix();
        if let Err(error) = self.persist(&tasks) {
            tasks.get_mut(id).unwrap().info = previous;
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn program_pause_requested(&self, id: &str) -> Result<bool, TaskError> {
        let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks
            .get(id)
            .map(|task| task.pause_requested)
            .ok_or_else(|| TaskError::NotFound(id.into()))
    }

    pub(crate) fn program_cancelled(&self, id: &str) -> bool {
        self.tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .is_none_or(|task| {
                task.cancel_flag.load(Ordering::Acquire) || task.info.state.is_terminal()
            })
    }

    /// Pause is cooperative: the current native step finishes; resource locks remain held.
    pub fn pause_program(&self, id: &str, pause: bool) -> Result<(), TaskError> {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        let task = tasks
            .get_mut(id)
            .ok_or_else(|| TaskError::NotFound(id.into()))?;
        if task.info.state != TaskState::Running
            || task.info.execution.is_none()
            || task.cancel_flag.load(Ordering::Acquire)
        {
            return Err(TaskError::InvalidTransition {
                from: task.info.state,
                to: TaskState::Running,
            });
        }
        let previous = task.info.clone();
        let old_pause = task.pause_requested;
        task.pause_requested = pause;
        task.info.execution.as_mut().unwrap().pause_requested = pause;
        if let Err(error) = self.persist(&tasks) {
            let task = tasks.get_mut(id).unwrap();
            task.info = previous;
            task.pause_requested = old_pause;
            return Err(error);
        }
        Ok(())
    }

    /// Updates task progress without changing state.
    pub fn update_progress(
        &self,
        task_id: &str,
        progress: impl Into<String>,
    ) -> Result<(), TaskError> {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| TaskError::NotFound(task_id.to_string()))?;
        let previous = task.info.clone();
        task.info.progress = Some(progress.into());
        task.info.updated_at = now_unix();
        if let Err(err) = self.persist(&tasks) {
            tasks.get_mut(task_id).unwrap().info = previous;
            return Err(err);
        }
        Ok(())
    }

    /// Transitions a task to `next_state`. Releases any locked resources if `next_state` is terminal.
    pub fn update_state(
        &self,
        task_id: &str,
        next_state: TaskState,
        progress: Option<String>,
    ) -> Result<(), TaskError> {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| TaskError::NotFound(task_id.to_string()))?;

        if !task.info.state.can_transition_to(next_state) {
            return Err(TaskError::InvalidTransition {
                from: task.info.state,
                to: next_state,
            });
        }

        let previous = task.info.clone();
        task.info.state = next_state;
        if progress.is_some() {
            task.info.progress = progress;
        }
        task.info.updated_at = now_unix();
        if let Err(err) = self.persist(&tasks) {
            tasks.get_mut(task_id).unwrap().info = previous;
            return Err(err);
        }

        if next_state.is_terminal() {
            let task = tasks.get(task_id).unwrap();
            let mut locks = self.path_locks.lock().unwrap_or_else(|e| e.into_inner());
            for path in &task.locked_paths {
                if locks.get(path).is_some_and(|holder| holder == task_id) {
                    locks.remove(path);
                }
            }
        }

        Ok(())
    }

    /// Requests cancellation. Running workers retain their locks until they acknowledge completion.
    pub fn cancel_task(&self, task_id: &str) -> bool {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(task) = tasks.get_mut(task_id) {
            if task.info.state.is_active() {
                task.cancel_flag.store(true, Ordering::Relaxed);
                let executing = matches!(
                    task.info.state,
                    TaskState::Running | TaskState::Preparing | TaskState::Verifying
                );
                if executing {
                    task.info.progress = Some("Cancellation requested".into());
                    task.info.updated_at = now_unix();
                    if let Err(err) = self.persist(&tasks) {
                        eprintln!("[tasks] {err}");
                    }
                    return true;
                }
                task.info.state = TaskState::Cancelled;
                task.info.updated_at = now_unix();

                let mut locks = self.path_locks.lock().unwrap_or_else(|e| e.into_inner());
                for path in &task.locked_paths {
                    if locks.get(path).is_some_and(|holder| holder == task_id) {
                        locks.remove(path);
                    }
                }
                if let Err(err) = self.persist(&tasks) {
                    eprintln!("[tasks] {err}");
                }
                return true;
            }
        }
        false
    }

    /// Signals cancellation for all currently active tasks.
    pub fn cancel_all(&self) {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        let mut locks = self.path_locks.lock().unwrap_or_else(|e| e.into_inner());
        for task in tasks.values_mut() {
            if task.info.state.is_active() {
                task.cancel_flag.store(true, Ordering::Relaxed);
                if matches!(
                    task.info.state,
                    TaskState::Running | TaskState::Preparing | TaskState::Verifying
                ) {
                    task.info.progress = Some("Cancellation requested".into());
                    task.info.updated_at = now_unix();
                    continue;
                }
                task.info.state = TaskState::Cancelled;
                task.info.updated_at = now_unix();
                for path in &task.locked_paths {
                    if locks
                        .get(path)
                        .is_some_and(|holder| holder == &task.info.id)
                    {
                        locks.remove(path);
                    }
                }
            }
        }
        if let Err(err) = self.persist(&tasks) {
            eprintln!("[tasks] {err}");
        }
    }

    /// Returns the info for a task, if it exists.
    pub fn get_task(&self, task_id: &str) -> Option<TaskInfo> {
        let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks.get(task_id).map(|t| t.info.clone())
    }

    /// Lists all currently active tasks.
    pub fn list_active_tasks(&self) -> Vec<TaskInfo> {
        let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks
            .values()
            .filter(|t| t.info.state.is_active() || t.info.state == TaskState::RecoveryRequired)
            .map(|t| t.info.clone())
            .collect()
    }

    /// Lists all tracked tasks (active and completed).
    pub fn list_all_tasks(&self) -> Vec<TaskInfo> {
        let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks.values().map(|t| t.info.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovered_resources_are_reserved_atomically_and_released_on_completion() {
        let coordinator = TaskCoordinator::new();
        let dir = tempfile::tempdir().unwrap();
        let one = dir.path().join("one");
        let two = dir.path().join("two");
        let (a, _) = coordinator
            .create_task("a".into(), TaskState::Running, std::slice::from_ref(&one))
            .unwrap();
        let (b, _) = coordinator
            .create_task("b".into(), TaskState::Running, std::slice::from_ref(&two))
            .unwrap();
        assert!(coordinator
            .reserve_resources(&a, &[dir.path().join("free"), two.clone()])
            .is_err());
        assert_eq!(coordinator.get_task(&a).unwrap().resources.len(), 1);
        assert!(coordinator
            .create_task(
                "free".into(),
                TaskState::Running,
                &[dir.path().join("free")]
            )
            .is_ok());
        coordinator
            .update_state(&b, TaskState::Completed, None)
            .unwrap();
        coordinator
            .reserve_resources(&a, std::slice::from_ref(&two))
            .unwrap();
        assert!(coordinator
            .create_task(
                "blocked".into(),
                TaskState::Running,
                std::slice::from_ref(&two)
            )
            .is_err());
        coordinator
            .update_state(&a, TaskState::Completed, None)
            .unwrap();
        assert!(coordinator
            .create_task("released".into(), TaskState::Running, &[two])
            .is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn restart_preserves_completed_tasks_and_marks_running_work_for_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tasks.dpapi");
        let coordinator = TaskCoordinator::open(path.clone()).unwrap();
        let (done, _) = coordinator
            .create_task("finished inspection".into(), TaskState::Running, &[])
            .unwrap();
        coordinator
            .update_state(&done, TaskState::Completed, Some("verified".into()))
            .unwrap();
        let (running, _) = coordinator
            .create_task(
                "copy in progress".into(),
                TaskState::Running,
                &[dir.path().join("output")],
            )
            .unwrap();
        drop(coordinator);
        let restored = TaskCoordinator::open(path.clone()).unwrap();
        assert_eq!(
            restored.get_task(&done).unwrap().state,
            TaskState::Completed
        );
        let interrupted = restored.get_task(&running).unwrap();
        assert_eq!(interrupted.state, TaskState::RecoveryRequired);
        assert!(interrupted
            .progress
            .unwrap()
            .contains("nothing was replayed"));
        assert!(!dir.path().join("output").exists());
        assert_eq!(restored.list_active_tasks().len(), 1);
        assert!(
            !String::from_utf8_lossy(&std::fs::read(&path).unwrap()).contains("copy in progress")
        );
        drop(restored);
        assert_eq!(
            TaskCoordinator::open(path)
                .unwrap()
                .get_task(&running)
                .unwrap()
                .state,
            TaskState::RecoveryRequired
        );
    }

    #[cfg(windows)]
    #[test]
    fn unreadable_task_journal_is_not_silently_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tasks.dpapi");
        std::fs::write(&path, b"corrupt journal").unwrap();
        assert!(matches!(
            TaskCoordinator::open(path.clone()),
            Err(TaskError::Persistence(_))
        ));
        assert_eq!(std::fs::read(path).unwrap(), b"corrupt journal");
    }

    #[cfg(windows)]
    #[test]
    fn failed_task_persistence_does_not_leave_a_phantom_task_or_resource_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tasks.dpapi");
        let coordinator = TaskCoordinator::open(path.clone()).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        let resource = dir.path().join("file");
        assert!(coordinator
            .create_task(
                "blocked".into(),
                TaskState::Running,
                std::slice::from_ref(&resource)
            )
            .is_err());
        assert!(coordinator.list_all_tasks().is_empty());
        std::fs::remove_dir(path).unwrap();
        assert!(coordinator
            .create_task("retry".into(), TaskState::Running, &[resource])
            .is_ok());
    }

    #[test]
    fn validates_task_state_transitions() {
        assert!(TaskState::Queued.can_transition_to(TaskState::Running));
        assert!(TaskState::Running.can_transition_to(TaskState::Verifying));
        assert!(TaskState::Verifying.can_transition_to(TaskState::Completed));

        // Terminal states cannot transition to new states
        assert!(!TaskState::Completed.can_transition_to(TaskState::Running));
        assert!(!TaskState::Cancelled.can_transition_to(TaskState::Running));

        // Any active state can cancel or fail
        assert!(TaskState::Running.can_transition_to(TaskState::Cancelled));
        assert!(TaskState::Preparing.can_transition_to(TaskState::Failed));
    }

    #[test]
    fn prevents_concurrent_resource_conflicts() {
        let coordinator = TaskCoordinator::new();
        let target = PathBuf::from("C:\\test\\file.txt");

        let (t1, _) = coordinator
            .create_task(
                "Move file".into(),
                TaskState::Running,
                std::slice::from_ref(&target),
            )
            .unwrap();

        let conflict = coordinator.create_task(
            "Another move".into(),
            TaskState::Running,
            std::slice::from_ref(&target),
        );
        assert!(matches!(conflict, Err(TaskError::ResourceConflict { .. })));

        // Completing t1 releases the lock
        coordinator
            .update_state(&t1, TaskState::Completed, None)
            .unwrap();

        let t2 = coordinator.create_task("Another move".into(), TaskState::Running, &[target]);
        assert!(t2.is_ok());
    }

    #[test]
    fn cancels_individual_task_without_affecting_others() {
        let coordinator = TaskCoordinator::new();
        let (t1, c1) = coordinator
            .create_task("Task 1".into(), TaskState::Running, &[])
            .unwrap();
        let (t2, c2) = coordinator
            .create_task("Task 2".into(), TaskState::Running, &[])
            .unwrap();

        assert!(!c1.load(Ordering::Relaxed));
        assert!(!c2.load(Ordering::Relaxed));

        assert!(coordinator.cancel_task(&t1));
        assert!(c1.load(Ordering::Relaxed));
        assert!(!c2.load(Ordering::Relaxed));

        let task1 = coordinator.get_task(&t1).unwrap();
        let task2 = coordinator.get_task(&t2).unwrap();
        assert_eq!(task1.state, TaskState::Running);
        coordinator
            .update_state(&t1, TaskState::Cancelled, None)
            .unwrap();
        assert_eq!(task2.state, TaskState::Running);
    }

    #[test]
    fn cancel_all_cancels_all_active_tasks() {
        let coordinator = TaskCoordinator::new();
        let (t1, c1) = coordinator
            .create_task("Task 1".into(), TaskState::Running, &[])
            .unwrap();
        let (t2, c2) = coordinator
            .create_task("Task 2".into(), TaskState::Running, &[])
            .unwrap();

        coordinator.cancel_all();
        assert!(c1.load(Ordering::Relaxed));
        assert!(c2.load(Ordering::Relaxed));
        assert_eq!(
            {
                coordinator
                    .update_state(&t1, TaskState::Cancelled, None)
                    .unwrap();
                coordinator.get_task(&t1).unwrap().state
            },
            TaskState::Cancelled
        );
        assert_eq!(
            {
                coordinator
                    .update_state(&t2, TaskState::Cancelled, None)
                    .unwrap();
                coordinator.get_task(&t2).unwrap().state
            },
            TaskState::Cancelled
        );
    }

    #[test]
    fn cancelled_worker_keeps_lock_until_it_stops() {
        let coordinator = TaskCoordinator::new();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("file");
        let (id, cancel) = coordinator
            .create_task(
                "copy".into(),
                TaskState::Running,
                std::slice::from_ref(&path),
            )
            .unwrap();
        assert!(coordinator.cancel_task(&id));
        assert!(cancel.load(Ordering::Relaxed));
        assert!(coordinator
            .create_task(
                "other".into(),
                TaskState::Running,
                std::slice::from_ref(&path)
            )
            .is_err());
        coordinator
            .update_state(&id, TaskState::Cancelled, None)
            .unwrap();
        let (_next, _) = coordinator
            .create_task(
                "other".into(),
                TaskState::Running,
                std::slice::from_ref(&path),
            )
            .unwrap();
        coordinator
            .update_state(&id, TaskState::Cancelled, None)
            .unwrap();
        assert!(coordinator
            .create_task("third".into(), TaskState::Running, &[path])
            .is_err());
    }

    #[test]
    fn parent_and_child_paths_conflict() {
        let coordinator = TaskCoordinator::new();
        let root = tempfile::tempdir().unwrap();
        coordinator
            .create_task(
                "folder".into(),
                TaskState::Running,
                &[root.path().to_path_buf()],
            )
            .unwrap();
        assert!(coordinator
            .create_task(
                "child".into(),
                TaskState::Running,
                &[root.path().join("child")]
            )
            .is_err());
    }
}
