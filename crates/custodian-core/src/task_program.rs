//! Bounded native programs. Plans and recorded grants are descriptions, never authority.
//! Adapters must recheck native permission and verify effects; no script/text execution.
use crate::coordinator::{TaskCoordinator, TaskState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Discover,
    Read,
    Write,
    App,
    Verify,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
pub enum Value {
    Bool(bool),
    Number(u64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub step: usize,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Predicate {
    pub fact: Reference,
    pub equals: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub title: String,
    pub kind: Kind,
    /// Native registry key, interpreted exclusively by the chosen adapter.
    pub capability: String,
    pub dependencies: Vec<usize>,
    pub inputs: BTreeMap<String, Reference>,
    pub when: Option<Predicate>,
    /// Extra attempts, permitted for observation steps only; never retry writes/apps.
    pub retries: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    pub goal: String,
    pub constraints: Vec<String>,
    pub scoped_grants: Vec<String>,
    pub steps: Vec<Step>,
    pub completion: Vec<Predicate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailureClass {
    RetryableObservation,
    StaleWorld,
    MissingAuthority,
    UnavailableCapability,
    Unrecoverable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    pub class: FailureClass,
    pub detail: String,
}
impl Failure {
    pub fn new(class: FailureClass, detail: impl Into<String>) -> Self {
        Self {
            class,
            detail: detail.into(),
        }
    }
}

pub type Facts = BTreeMap<String, Value>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub facts: Facts,
    pub receipts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub program: Program,
    pub outputs: BTreeMap<usize, Outcome>,
    pub skipped: Vec<usize>,
    pub in_flight: Option<usize>,
    pub attempts: BTreeMap<usize, u8>,
    pub failure: Option<Failure>,
    pub pause_requested: bool,
    pub paused: bool,
    pub verified: bool,
}

/// Implementations are native code. A serialized grant string cannot authorize a call.
pub trait Adapter {
    fn authorize(&mut self, step: &Step) -> Result<(), Failure>;
    fn execute(
        &mut self,
        step: &Step,
        inputs: &Facts,
        cancel: &AtomicBool,
    ) -> Result<Outcome, Failure>;
}

impl Program {
    pub fn validate(&self) -> Result<(), Failure> {
        let bad = || {
            Failure::new(
                FailureClass::Unrecoverable,
                "Invalid or unbounded task program",
            )
        };
        if self.goal.is_empty()
            || self.goal.len() > 4096
            || self.steps.is_empty()
            || self.steps.len() > 64
            || self.completion.is_empty()
            || self.completion.len() > 64
            || self.constraints.len() > 32
            || self.scoped_grants.len() > 32
            || self
                .constraints
                .iter()
                .chain(&self.scoped_grants)
                .any(|s| s.len() > 4096)
        {
            return Err(bad());
        }
        for (i, step) in self.steps.iter().enumerate() {
            if step.title.is_empty()
                || step.title.len() > 512
                || step.capability.is_empty()
                || step.capability.len() > 128
                || step.dependencies.len() > 64
                || step.inputs.len() > 32
                || step.retries > 2
                || (step.retries > 0 && matches!(step.kind, Kind::Write | Kind::App))
                || step.dependencies.iter().any(|d| *d >= i)
            {
                return Err(bad());
            }
            for reference in step
                .inputs
                .values()
                .chain(step.when.iter().map(|p| &p.fact))
            {
                if reference.key.len() > 128 || !step.dependencies.contains(&reference.step) {
                    return Err(bad());
                }
            }
        }
        for test in &self.completion {
            if test.fact.step >= self.steps.len() || self.steps[test.fact.step].kind != Kind::Verify
            {
                return Err(bad());
            }
        }
        Ok(())
    }
}

fn fact<'a>(checkpoint: &'a Checkpoint, reference: &Reference) -> Option<&'a Value> {
    checkpoint
        .outputs
        .get(&reference.step)?
        .facts
        .get(&reference.key)
}
fn matches(checkpoint: &Checkpoint, predicate: &Predicate) -> bool {
    fact(checkpoint, &predicate.fact) == Some(&predicate.equals)
}

impl TaskCoordinator {
    /// Runs an already admitted task. Checkpoints are durable before each adapter call.
    /// Restart never resumes this program; old facts are evidence, not replay instructions.
    pub fn run_program(
        &self,
        id: &str,
        program: Program,
        cancel: &AtomicBool,
        adapter: &mut dyn Adapter,
    ) -> Result<Checkpoint, Failure> {
        program.validate()?;
        let mut checkpoint = Checkpoint {
            program,
            outputs: BTreeMap::new(),
            skipped: vec![],
            in_flight: None,
            attempts: BTreeMap::new(),
            failure: None,
            pause_requested: false,
            paused: false,
            verified: false,
        };
        self.start_program(id, &checkpoint).map_err(persistence)?;
        let result = (|| {
            for index in 0..checkpoint.program.steps.len() {
                self.program_boundary(id, cancel, &mut checkpoint)?;
                let step = checkpoint.program.steps[index].clone();
                if step
                    .when
                    .as_ref()
                    .is_some_and(|p| fact(&checkpoint, &p.fact).is_none())
                {
                    return Err(Failure::new(
                        FailureClass::StaleWorld,
                        "Branch observation is missing",
                    ));
                }
                if step.when.as_ref().is_some_and(|p| !matches(&checkpoint, p)) {
                    checkpoint.skipped.push(index);
                    self.program_checkpoint(id, &checkpoint)
                        .map_err(persistence)?;
                    continue;
                }
                if step
                    .dependencies
                    .iter()
                    .any(|d| !checkpoint.outputs.contains_key(d))
                {
                    return Err(Failure::new(
                        FailureClass::StaleWorld,
                        "A required step has no verified output",
                    ));
                }
                let inputs = step
                    .inputs
                    .iter()
                    .map(|(key, reference)| {
                        fact(&checkpoint, reference)
                            .cloned()
                            .map(|value| (key.clone(), value))
                            .ok_or_else(|| {
                                Failure::new(FailureClass::StaleWorld, "A required fact is missing")
                            })
                    })
                    .collect::<Result<Facts, _>>()?;
                for attempt in 0..=step.retries {
                    self.program_boundary(id, cancel, &mut checkpoint)?;
                    adapter.authorize(&step)?;
                    checkpoint.in_flight = Some(index);
                    checkpoint.attempts.insert(index, attempt + 1);
                    self.program_checkpoint(id, &checkpoint)
                        .map_err(persistence)?;
                    match adapter.execute(&step, &inputs, cancel) {
                        Ok(outcome) => {
                            if outcome.facts.len() > 32
                                || (matches!(step.kind, Kind::Write | Kind::App)
                                    && outcome.receipts.is_empty())
                                || outcome.receipts.len() > 64
                                || outcome.receipts.iter().any(|r| r.len() > 4096)
                                || outcome.facts.iter().any(|(k, v)| {
                                    k.len() > 128 || matches!(v,Value::Text(s) if s.len()>4096)
                                })
                            {
                                return Err(Failure::new(
                                    FailureClass::Unrecoverable,
                                    "Adapter output exceeded evidence limits or omitted an effect receipt",
                                ));
                            }
                            checkpoint.outputs.insert(index, outcome);
                            checkpoint.in_flight = None;
                            self.program_checkpoint(id, &checkpoint)
                                .map_err(persistence)?;
                            break;
                        }
                        Err(error)
                            if error.class == FailureClass::RetryableObservation
                                && attempt < step.retries =>
                        {
                            checkpoint.in_flight = None;
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
            self.program_boundary(id, cancel, &mut checkpoint)?;
            if !checkpoint
                .program
                .completion
                .iter()
                .all(|test| matches(&checkpoint, test))
            {
                return Err(Failure::new(
                    FailureClass::StaleWorld,
                    "Completion tests did not pass",
                ));
            }
            checkpoint.verified = true;
            Ok(())
        })();
        if let Err(error) = &result {
            checkpoint.failure = Some(error.clone());
        }
        self.program_checkpoint(id, &checkpoint)
            .map_err(persistence)?;
        let effects = checkpoint
            .outputs
            .iter()
            .any(|(i, _)| matches!(checkpoint.program.steps[*i].kind, Kind::Write | Kind::App));
        let uncertain_effect = checkpoint
            .in_flight
            .is_some_and(|i| matches!(checkpoint.program.steps[i].kind, Kind::Write | Kind::App));
        let state = if result.is_ok() {
            TaskState::Completed
        } else if uncertain_effect {
            TaskState::RecoveryRequired
        } else if effects {
            TaskState::Partial
        } else if cancel.load(Ordering::Acquire)
            || self.program_cancelled(id)
            || !crate::privacy::request_is_current()
        {
            TaskState::Cancelled
        } else {
            TaskState::Failed
        };
        self.update_state(
            id,
            state,
            Some(if checkpoint.verified {
                "All completion checks passed".into()
            } else {
                "Not completed. Inspect recorded effects and receipts before retrying.".into()
            }),
        )
        .map_err(persistence)?;
        result.map(|()| checkpoint)
    }

    fn program_boundary(
        &self,
        id: &str,
        cancel: &AtomicBool,
        checkpoint: &mut Checkpoint,
    ) -> Result<(), Failure> {
        loop {
            if cancel.load(Ordering::Acquire)
                || self.program_cancelled(id)
                || !crate::privacy::request_is_current()
            {
                return Err(Failure::new(
                    FailureClass::MissingAuthority,
                    "Task stopped or request authority changed",
                ));
            }
            let paused = self.program_pause_requested(id).map_err(persistence)?;
            if checkpoint.paused != paused {
                checkpoint.pause_requested = paused;
                checkpoint.paused = paused;
                self.program_checkpoint(id, checkpoint)
                    .map_err(persistence)?;
            }
            if !paused {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
    }
}
fn persistence(error: crate::coordinator::TaskError) -> Failure {
    Failure::new(FailureClass::Unrecoverable, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference(step: usize) -> Reference {
        Reference {
            step,
            key: "ok".into(),
        }
    }
    fn predicate(step: usize, value: bool) -> Predicate {
        Predicate {
            fact: reference(step),
            equals: Value::Bool(value),
        }
    }
    fn step(kind: Kind, index: usize) -> Step {
        Step {
            title: format!("Mock step {index}"),
            kind,
            capability: format!("mock.{index}"),
            dependencies: if index == 0 { vec![] } else { vec![index - 1] },
            inputs: if index == 0 {
                BTreeMap::new()
            } else {
                [("previous".into(), reference(index - 1))].into()
            },
            when: None,
            retries: 0,
        }
    }
    fn program(kinds: &[Kind]) -> Program {
        Program {
            goal: "Deterministic mock fixture; no real app control".into(),
            constraints: vec![],
            scoped_grants: vec!["Descriptive mock permission; cannot grant native authority".into()],
            steps: kinds.iter().enumerate().map(|(i, k)| step(*k, i)).collect(),
            completion: vec![predicate(kinds.len() - 1, true)],
        }
    }
    #[derive(Default)]
    struct Mock {
        calls: Vec<String>,
        auth: usize,
        fail: Option<FailureClass>,
        failures_left: usize,
        value: bool,
    }
    impl Adapter for Mock {
        fn authorize(&mut self, step: &Step) -> Result<(), Failure> {
            self.auth += 1;
            if !step.capability.starts_with("mock.") {
                return Err(Failure::new(
                    FailureClass::UnavailableCapability,
                    "No real app adapter registered",
                ));
            }
            Ok(())
        }
        fn execute(
            &mut self,
            step: &Step,
            inputs: &Facts,
            _: &AtomicBool,
        ) -> Result<Outcome, Failure> {
            self.calls.push(step.capability.clone());
            if self.failures_left > 0 {
                self.failures_left -= 1;
                return Err(Failure::new(self.fail.unwrap(), "Injected mock failure"));
            }
            if !inputs.is_empty() {
                assert_eq!(inputs.get("previous"), Some(&Value::Bool(self.value)));
            }
            Ok(Outcome {
                facts: [("ok".into(), Value::Bool(self.value))].into(),
                receipts: if matches!(step.kind, Kind::Write | Kind::App) {
                    vec!["mock.receipt".into()]
                } else {
                    vec![]
                },
            })
        }
    }
    fn task(coordinator: &TaskCoordinator) -> (String, std::sync::Arc<AtomicBool>) {
        coordinator
            .create_task("Fixture program".into(), TaskState::Running, &[])
            .unwrap()
    }
    #[test]
    fn composes_all_five_types_with_bound_inputs_and_explicit_completion() {
        let coordinator = TaskCoordinator::new();
        let (id, cancel) = task(&coordinator);
        let mut mock = Mock {
            value: true,
            ..Mock::default()
        };
        let result = coordinator
            .run_program(
                &id,
                program(&[
                    Kind::Discover,
                    Kind::Read,
                    Kind::Write,
                    Kind::App,
                    Kind::Verify,
                ]),
                &cancel,
                &mut mock,
            )
            .unwrap();
        assert_eq!(result.outputs.len(), 5);
        assert!(result.verified);
        assert_eq!(mock.auth, 5);
        assert_eq!(
            coordinator.get_task(&id).unwrap().state,
            TaskState::Completed
        );
    }
    #[test]
    fn rejects_cycles_forward_inputs_write_retries_and_nonverifier_completion() {
        let valid = program(&[Kind::Read, Kind::Verify]);
        let mut bad = valid.clone();
        bad.steps[0].dependencies.push(1);
        assert!(bad.validate().is_err());
        let mut bad = valid.clone();
        bad.steps[0].inputs.insert("x".into(), reference(1));
        assert!(bad.validate().is_err());
        let mut bad = valid.clone();
        bad.steps[0].kind = Kind::Write;
        bad.steps[0].retries = 1;
        assert!(bad.validate().is_err());
        let mut bad = valid.clone();
        bad.completion = vec![predicate(0, true)];
        assert!(bad.validate().is_err());
        let mut bad = valid;
        bad.steps[0].retries = 3;
        assert!(bad.validate().is_err());
    }
    #[test]
    fn retries_only_observations_with_fresh_authorization_and_hard_bound() {
        let coordinator = TaskCoordinator::new();
        let (id, cancel) = task(&coordinator);
        let mut p = program(&[Kind::Read, Kind::Verify]);
        p.steps[0].retries = 2;
        let mut mock = Mock {
            value: true,
            fail: Some(FailureClass::RetryableObservation),
            failures_left: 2,
            ..Mock::default()
        };
        let result = coordinator.run_program(&id, p, &cancel, &mut mock).unwrap();
        assert_eq!(mock.auth, 4);
        assert_eq!(result.attempts[&0], 3);
        let (id, cancel) = task(&coordinator);
        let mut p = program(&[Kind::Read, Kind::Verify]);
        p.steps[0].retries = 2;
        mock.failures_left = 20;
        mock.calls.clear();
        assert!(coordinator.run_program(&id, p, &cancel, &mut mock).is_err());
        assert_eq!(mock.calls.len(), 3);
    }
    #[test]
    fn uncertain_write_is_never_retried_or_claimed_complete() {
        let coordinator = TaskCoordinator::new();
        let (id, cancel) = task(&coordinator);
        let mut mock = Mock {
            fail: Some(FailureClass::RetryableObservation),
            failures_left: 1,
            ..Mock::default()
        };
        assert!(coordinator
            .run_program(
                &id,
                program(&[Kind::Write, Kind::Verify]),
                &cancel,
                &mut mock
            )
            .is_err());
        assert_eq!(mock.calls.len(), 1);
        let saved = coordinator.get_task(&id).unwrap();
        assert_eq!(saved.state, TaskState::RecoveryRequired);
        assert_eq!(saved.execution.unwrap().in_flight, Some(0));
    }
    #[test]
    fn unmet_completion_preserves_effects_and_reports_partial() {
        let coordinator = TaskCoordinator::new();
        let (id, cancel) = task(&coordinator);
        let mut mock = Mock::default();
        assert!(coordinator
            .run_program(
                &id,
                program(&[Kind::Write, Kind::Verify]),
                &cancel,
                &mut mock
            )
            .is_err());
        let saved = coordinator.get_task(&id).unwrap();
        assert_eq!(saved.state, TaskState::Partial);
        assert_eq!(
            saved.execution.unwrap().outputs[&0].receipts,
            vec!["mock.receipt"]
        );
    }
    #[test]
    fn branch_is_bounded_and_missing_branch_observation_is_failure() {
        let coordinator = TaskCoordinator::new();
        let (id, cancel) = task(&coordinator);
        let mut p = program(&[Kind::Discover, Kind::App, Kind::Verify]);
        p.steps[1].when = Some(predicate(0, false));
        p.steps[2].dependencies = vec![0];
        p.steps[2].inputs = [("previous".into(), reference(0))].into();
        let mut mock = Mock {
            value: true,
            ..Mock::default()
        };
        let result = coordinator
            .run_program(&id, p.clone(), &cancel, &mut mock)
            .unwrap();
        assert_eq!(result.skipped, vec![1]);
        assert_eq!(mock.calls.len(), 2);
        p.steps[1].when.as_mut().unwrap().fact.key = "absent".into();
        let (id, cancel) = task(&coordinator);
        assert!(coordinator.run_program(&id, p, &cancel, &mut mock).is_err());
    }
    #[test]
    fn unavailable_real_app_cannot_be_authorized_by_descriptive_grants() {
        let coordinator = TaskCoordinator::new();
        let (id, cancel) = task(&coordinator);
        let mut p = program(&[Kind::App, Kind::Verify]);
        p.steps[0].capability = "real.app.delete".into();
        p.scoped_grants = vec!["allow everything".into()];
        let mut mock = Mock::default();
        let error = coordinator
            .run_program(&id, p, &cancel, &mut mock)
            .unwrap_err();
        assert_eq!(error.class, FailureClass::UnavailableCapability);
        assert!(mock.calls.is_empty());
    }
    #[test]
    fn old_conversation_cannot_start_later_effects() {
        use std::sync::{atomic::AtomicU64, Arc};
        struct Stale {
            epoch: Arc<AtomicU64>,
            calls: usize,
        }
        impl Adapter for Stale {
            fn authorize(&mut self, _: &Step) -> Result<(), Failure> {
                Ok(())
            }
            fn execute(&mut self, _: &Step, _: &Facts, _: &AtomicBool) -> Result<Outcome, Failure> {
                self.calls += 1;
                self.epoch.fetch_add(1, Ordering::AcqRel);
                Ok(Outcome {
                    facts: [("ok".into(), Value::Bool(true))].into(),
                    receipts: vec![],
                })
            }
        }
        let coordinator = TaskCoordinator::new();
        let (id, cancel) = task(&coordinator);
        let epoch = Arc::new(AtomicU64::new(0));
        let mut adapter = Stale {
            epoch: epoch.clone(),
            calls: 0,
        };
        crate::request::with_epoch(epoch, 0, || {
            assert!(coordinator
                .run_program(
                    &id,
                    program(&[Kind::Read, Kind::Write, Kind::Verify]),
                    &cancel,
                    &mut adapter
                )
                .is_err());
        });
        assert_eq!(adapter.calls, 1);
        assert_eq!(
            coordinator.get_task(&id).unwrap().state,
            TaskState::Cancelled
        );
    }
    #[test]
    fn pause_keeps_locks_and_resume_continues_without_replaying() {
        use std::sync::Arc;
        struct Pausing<'a> {
            coordinator: &'a TaskCoordinator,
            id: &'a str,
            mock: Mock,
        }
        impl Adapter for Pausing<'_> {
            fn authorize(&mut self, s: &Step) -> Result<(), Failure> {
                self.mock.authorize(s)
            }
            fn execute(&mut self, s: &Step, i: &Facts, c: &AtomicBool) -> Result<Outcome, Failure> {
                let out = self.mock.execute(s, i, c)?;
                if self.mock.calls.len() == 1 {
                    self.coordinator.pause_program(self.id, true).unwrap();
                }
                Ok(out)
            }
        }
        let coordinator = Arc::new(TaskCoordinator::new());
        let dir = tempfile::tempdir().unwrap();
        let resource = dir.path().join("output");
        let (id, cancel) = coordinator
            .create_task(
                "Pause fixture".into(),
                TaskState::Running,
                std::slice::from_ref(&resource),
            )
            .unwrap();
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                let mut adapter = Pausing {
                    coordinator: &coordinator,
                    id: &id,
                    mock: Mock {
                        value: true,
                        ..Mock::default()
                    },
                };
                coordinator
                    .run_program(
                        &id,
                        program(&[Kind::Read, Kind::Verify]),
                        &cancel,
                        &mut adapter,
                    )
                    .unwrap();
                assert_eq!(adapter.mock.calls.len(), 2);
            });
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !coordinator
                .get_task(&id)
                .unwrap()
                .execution
                .is_some_and(|e| e.paused)
            {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            assert!(coordinator
                .create_task(
                    "Conflict".into(),
                    TaskState::Running,
                    std::slice::from_ref(&resource)
                )
                .is_err());
            coordinator.pause_program(&id, false).unwrap();
            worker.join().unwrap();
        });
        assert_eq!(
            coordinator.get_task(&id).unwrap().state,
            TaskState::Completed
        );
    }
    #[test]
    fn task_stop_is_checked_even_if_caller_supplies_an_unrelated_cancel_flag() {
        struct Cancelling<'a> {
            coordinator: &'a TaskCoordinator,
            id: &'a str,
            calls: usize,
        }
        impl Adapter for Cancelling<'_> {
            fn authorize(&mut self, _: &Step) -> Result<(), Failure> {
                Ok(())
            }
            fn execute(&mut self, _: &Step, _: &Facts, _: &AtomicBool) -> Result<Outcome, Failure> {
                self.calls += 1;
                self.coordinator.cancel_task(self.id);
                Ok(Outcome {
                    facts: [("ok".into(), Value::Bool(true))].into(),
                    receipts: vec![],
                })
            }
        }
        let coordinator = TaskCoordinator::new();
        let (id, _) = task(&coordinator);
        let (other, other_cancel) = task(&coordinator);
        let mut adapter = Cancelling {
            coordinator: &coordinator,
            id: &id,
            calls: 0,
        };
        assert!(coordinator
            .run_program(
                &id,
                program(&[Kind::Read, Kind::Write, Kind::Verify]),
                &AtomicBool::new(false),
                &mut adapter
            )
            .is_err());
        assert_eq!(adapter.calls, 1);
        assert!(!other_cancel.load(Ordering::Acquire));
        assert_eq!(
            coordinator.get_task(&other).unwrap().state,
            TaskState::Running
        );
    }
    #[cfg(windows)]
    #[test]
    fn active_program_restart_preserves_evidence_without_resume_authority() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tasks.dpapi");
        let coordinator = TaskCoordinator::open(path.clone()).unwrap();
        let (id, _) = task(&coordinator);
        let checkpoint = Checkpoint {
            program: program(&[Kind::Write, Kind::Verify]),
            outputs: BTreeMap::new(),
            skipped: vec![],
            in_flight: Some(0),
            attempts: [(0, 1)].into(),
            failure: None,
            pause_requested: true,
            paused: true,
            verified: false,
        };
        coordinator.start_program(&id, &checkpoint).unwrap();
        drop(coordinator);
        let restored = TaskCoordinator::open(path).unwrap();
        let task = restored.get_task(&id).unwrap();
        assert_eq!(task.state, TaskState::RecoveryRequired);
        let execution = task.execution.unwrap();
        assert_eq!(execution.in_flight, Some(0));
        assert!(!execution.paused);
        assert!(!execution.pause_requested);
        assert!(restored.pause_program(&id, false).is_err());
        assert!(!execution.verified);
    }
    #[test]
    fn effect_without_receipt_requires_recovery_instead_of_completion() {
        struct Unproven;
        impl Adapter for Unproven {
            fn authorize(&mut self, _: &Step) -> Result<(), Failure> {
                Ok(())
            }
            fn execute(&mut self, _: &Step, _: &Facts, _: &AtomicBool) -> Result<Outcome, Failure> {
                Ok(Outcome {
                    facts: [("ok".into(), Value::Bool(true))].into(),
                    receipts: vec![],
                })
            }
        }
        let coordinator = TaskCoordinator::new();
        let (id, cancel) = task(&coordinator);
        assert!(coordinator
            .run_program(
                &id,
                program(&[Kind::Write, Kind::Verify]),
                &cancel,
                &mut Unproven
            )
            .is_err());
        assert_eq!(
            coordinator.get_task(&id).unwrap().state,
            TaskState::RecoveryRequired
        );
    }
    #[cfg(windows)]
    #[test]
    fn restart_retains_inflight_evidence_and_never_replays_program() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("program.dpapi");
        let coordinator = TaskCoordinator::open(path.clone()).unwrap();
        let (id, cancel) = task(&coordinator);
        let mut mock = Mock {
            fail: Some(FailureClass::Unrecoverable),
            failures_left: 1,
            ..Mock::default()
        };
        assert!(coordinator
            .run_program(
                &id,
                program(&[Kind::Write, Kind::Verify]),
                &cancel,
                &mut mock
            )
            .is_err());
        let before = coordinator.get_task(&id).unwrap().execution;
        drop(coordinator);
        let restored = TaskCoordinator::open(path).unwrap();
        assert_eq!(restored.get_task(&id).unwrap().execution, before);
        assert!(restored
            .run_program(
                &id,
                program(&[Kind::Write, Kind::Verify]),
                &cancel,
                &mut mock
            )
            .is_err());
        assert_eq!(mock.calls.len(), 1);
    }
}
