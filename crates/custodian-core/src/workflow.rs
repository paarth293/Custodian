//! Bounded, composable file workflows. Model proposals never carry paths or grants.
use crate::{
    file_tasks::{CopyLimits, FileAction, FileTaskPlan, FileTasks, ReceiptState},
    routine::{CopyOperation, RoutinePolicy, FREE_RESERVE, MAX_BACKUP_BYTES},
    walk::{walk_files, FileEntry, WalkLimits},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub const MAX_FILES: usize = 20;
static WORKFLOWS: Mutex<()> = Mutex::new(());
fn invalid(message: &str) -> io::Error {
    io::Error::other(message)
}

fn canonical_id(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok_and(|value| value.to_string() == id)
}
fn receipt_matches(receipt: &crate::file_tasks::FileReceipt, step: &Step) -> bool {
    receipt.action == FileAction::Copy
        && receipt.source.as_ref() == Some(&step.source)
        && receipt.destination == step.destination
}
fn stopped(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) || !crate::privacy::request_is_current() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Workflow stopped; inspect its saved state before continuing",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Filter {
    Any,
    Documents,
    Images,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub filter: Filter,
    pub largest: bool,
}
impl Selection {
    pub fn steps(self) -> Vec<String> {
        let mut steps = vec!["discover".into()];
        match self.filter {
            Filter::Documents => steps.push("filter_documents".into()),
            Filter::Images => steps.push("filter_images".into()),
            Filter::Any => {}
        }
        steps.push(
            if self.largest {
                "select_largest"
            } else {
                "select_all"
            }
            .into(),
        );
        steps.extend(["copy".into(), "verify".into()]);
        steps
    }
    fn accepts(self, entry: &FileEntry) -> bool {
        let extension = entry
            .path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        match self.filter {
            Filter::Any => true,
            Filter::Documents => matches!(
                extension.as_str(),
                "pdf"
                    | "doc"
                    | "docx"
                    | "odt"
                    | "txt"
                    | "md"
                    | "rtf"
                    | "csv"
                    | "xlsx"
                    | "xls"
                    | "pptx"
                    | "ppt"
            ),
            Filter::Images => matches!(
                extension.as_str(),
                "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" | "heic" | "tif" | "tiff"
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub steps: Vec<String>,
    #[serde(default = "one_attempt")]
    pub attempts: u8,
}
fn one_attempt() -> u8 {
    1
}
impl Proposal {
    /// Compile accepted capabilities to streaming operators. The program's
    /// filter and selector drive execution only after the native goal matches.
    fn compile(&self, requested: Selection) -> io::Result<Selection> {
        self.validate(requested)?;
        Ok(Selection {
            filter: if self.steps.iter().any(|step| step == "filter_documents") {
                Filter::Documents
            } else if self.steps.iter().any(|step| step == "filter_images") {
                Filter::Images
            } else {
                Filter::Any
            },
            largest: self.steps.iter().any(|step| step == "select_largest"),
        })
    }
    pub fn validate(&self, requested: Selection) -> io::Result<()> {
        if !(1..=2).contains(&self.attempts) || self.steps != requested.steps() {
            return Err(invalid(
                "The proposed steps do not match the requested goal. Nothing was copied.",
            ));
        }
        Ok(())
    }
}

pub struct Prepared {
    plans: Vec<FileTaskPlan>,
    pub scanned: usize,
    pub excluded: usize,
    pub inaccessible: usize,
    revision: u64,
    selection: Selection,
    proposal: Proposal,
}

pub struct Goal {
    pub roots: Vec<PathBuf>,
    pub destination: PathBuf,
    pub selection: Selection,
}

/// Preparation is read-only. Conflicts, duplicate flattened names and total
/// budget overflow are refused before any copy, never silently truncated.
pub fn prepare(
    policy: &RoutinePolicy,
    revision: u64,
    goal: &Goal,
    proposal: Proposal,
    cancel: &AtomicBool,
    eligible: &dyn Fn(&Path) -> bool,
    progress: &mut dyn FnMut(usize),
) -> io::Result<Prepared> {
    let (roots, destination) = (&goal.roots, &goal.destination);
    policy.with_copy_grant(
        CopyOperation::Collection,
        revision,
        roots,
        destination,
        || Ok(()),
    )?;
    let selection = proposal.compile(goal.selection)?;
    policy.with_collection(revision, || Ok(()))?;
    stopped(cancel)?;
    crate::privacy::check(destination)?;
    let destination = destination.canonicalize()?;
    crate::privacy::check(&destination)?;
    if !destination.is_dir() || !eligible(&destination) || roots.is_empty() {
        return Err(invalid(
            "Choose an existing permitted destination and source",
        ));
    }
    let mut sources = Vec::new();
    for root in roots {
        crate::privacy::check(root)?;
        let root = root.canonicalize()?;
        crate::privacy::check(&root)?;
        if !root.is_dir() || root == destination {
            return Err(invalid("Choose distinct source and destination folders"));
        }
        if !sources.contains(&root) {
            sources.push(root);
        }
    }
    let mut entries: Vec<FileEntry> = Vec::new();
    let mut overflow = false;
    let stats = walk_files(
        &sources,
        WalkLimits::whole_system_read(),
        cancel,
        progress,
        &mut |entry| {
            if entry.path.starts_with(&destination)
                || !eligible(&entry.path)
                || !selection.accepts(&entry)
            {
                return;
            }
            if selection.largest {
                if entries.first().is_none_or(|old| {
                    entry.size > old.size || (entry.size == old.size && entry.path < old.path)
                }) {
                    entries.clear();
                    entries.push(entry);
                }
            } else if !entries.iter().any(|old| old.path == entry.path) {
                if entries.len() == MAX_FILES {
                    overflow = true;
                } else {
                    entries.push(entry);
                }
            }
        },
    );
    stopped(cancel)?;
    if stats.truncated || overflow {
        return Err(invalid("The collection exceeds 20 files or discovery was incomplete. Narrow the source; nothing was copied."));
    }
    if entries.is_empty() {
        return Err(invalid("No permitted matching files were found"));
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    let total = entries.iter().try_fold(0u64, |total, entry| {
        total
            .checked_add(entry.size)
            .ok_or_else(|| invalid("collection size overflow"))
    })?;
    if total > MAX_BACKUP_BYTES {
        return Err(invalid(
            "The complete collection exceeds 100 MiB. Nothing was copied.",
        ));
    }
    let mut plans: Vec<FileTaskPlan> = Vec::new();
    for entry in entries {
        stopped(cancel)?;
        let target = destination.join(
            entry
                .path
                .file_name()
                .ok_or_else(|| invalid("missing name"))?,
        );
        if plans.iter().any(|old| {
            old.destination.to_string_lossy().to_lowercase()
                == target.to_string_lossy().to_lowercase()
        }) {
            return Err(invalid("Two selected files have the same destination name. Narrow the source; nothing was copied."));
        }
        let plan = FileTaskPlan::prepare(FileAction::Copy, Some(&entry.path), &target)?;
        if plan.bytes() != entry.size
            || !eligible(plan.source.as_deref().unwrap())
            || !eligible(&plan.destination)
        {
            return Err(invalid("A selected file changed or is not permitted"));
        }
        plans.push(plan);
    }
    policy.with_collection(revision, || Ok(()))?;
    Ok(Prepared {
        plans,
        scanned: stats.files_seen,
        excluded: stats.excluded_count,
        inaccessible: stats.access_denied_count,
        revision,
        selection,
        proposal,
    })
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Prepared,
    Running,
    Verifying,
    Completed,
    Interrupted,
    Undoing,
    Undone,
    RecoveryRequired,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub receipt: String,
    pub source: PathBuf,
    pub destination: PathBuf,
    pub verified: bool,
    pub undone: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub created: u128,
    pub selection: Selection,
    pub proposal: Proposal,
    pub state: State,
    pub steps: Vec<Step>,
    pub detail: String,
}

pub struct WorkflowStore {
    root: PathBuf,
}
impl WorkflowStore {
    /// The existing write-ahead collection executor is one bounded write step.
    /// Pause waits for that step to finish; Stop is also checked between its copies.
    pub fn execute_coordinated(
        &self,
        task: (&crate::coordinator::TaskCoordinator, &str),
        policy: &RoutinePolicy,
        prepared: Prepared,
        files: &FileTasks,
        control: (&AtomicBool, &mut dyn FnMut(&Record)),
    ) -> io::Result<Record> {
        use crate::task_program::*;
        let resources = prepared
            .plans
            .iter()
            .flat_map(|plan| plan.source.iter().chain(std::iter::once(&plan.destination)))
            .cloned()
            .collect::<Vec<_>>();
        task.0
            .reserve_resources(task.1, &resources)
            .map_err(io::Error::other)?;
        struct Collection<'a> {
            store: &'a WorkflowStore,
            policy: &'a RoutinePolicy,
            prepared: Option<Prepared>,
            files: &'a FileTasks,
            revision: u64,
            progress: &'a mut dyn FnMut(&Record),
            record: Option<Record>,
        }
        impl Adapter for Collection<'_> {
            fn authorize(&mut self, step: &crate::task_program::Step) -> Result<(), Failure> {
                if !matches!(
                    (step.kind, step.capability.as_str()),
                    (Kind::Read, "collection-preflight")
                        | (Kind::Write, "collection-copy")
                        | (Kind::Verify, "collection-verify")
                ) {
                    return Err(Failure::new(
                        FailureClass::UnavailableCapability,
                        "Unregistered collection capability",
                    ));
                }
                self.policy
                    .with_collection(self.revision, || Ok(()))
                    .map_err(|e| Failure::new(FailureClass::MissingAuthority, e.to_string()))
            }
            fn execute(
                &mut self,
                step: &crate::task_program::Step,
                _inputs: &Facts,
                cancel: &AtomicBool,
            ) -> Result<Outcome, Failure> {
                let result: io::Result<Outcome> = (|| match step.capability.as_str() {
                    "collection-preflight" => {
                        let prepared = self
                            .prepared
                            .as_ref()
                            .ok_or_else(|| invalid("Missing prepared collection"))?;
                        for plan in &prepared.plans {
                            plan.validate()?;
                        }
                        Ok(Outcome {
                            facts: [("ready".into(), Value::Bool(true))].into(),
                            receipts: vec![],
                        })
                    }
                    "collection-copy" => {
                        let prepared = self
                            .prepared
                            .take()
                            .ok_or_else(|| invalid("Collection cannot be replayed"))?;
                        let record = self.store.execute(
                            self.policy,
                            prepared,
                            self.files,
                            cancel,
                            self.progress,
                        )?;
                        let receipts = record.steps.iter().map(|s| s.receipt.clone()).collect();
                        self.record = Some(record);
                        Ok(Outcome {
                            facts: [("copied".into(), Value::Bool(true))].into(),
                            receipts,
                        })
                    }
                    "collection-verify" => {
                        let record = self
                            .record
                            .as_ref()
                            .ok_or_else(|| invalid("Missing collection receipts"))?;
                        let receipts = self.files.receipts()?;
                        for step in &record.steps {
                            stopped(cancel)?;
                            crate::privacy::check(&step.source)?;
                            crate::privacy::check(&step.destination)?;
                            let receipt = receipts
                                .iter()
                                .find(|r| r.id == step.receipt)
                                .ok_or_else(|| invalid("Missing receipt"))?;
                            if !receipt_matches(receipt, step) {
                                return Err(invalid("Receipt mismatch"));
                            }
                            self.files.verify_copy(receipt)?;
                        }
                        Ok(Outcome {
                            facts: [("verified".into(), Value::Bool(true))].into(),
                            receipts: vec![record.id.clone()],
                        })
                    }
                    _ => Err(invalid("Unavailable capability")),
                })();
                result.map_err(|e| Failure::new(FailureClass::StaleWorld, e.to_string()))
            }
        }
        let reference = |step, key: &str| Reference {
            step,
            key: key.into(),
        };
        let steps = [
            (Kind::Read, "collection-preflight", "Check captured files"),
            (
                Kind::Write,
                "collection-copy",
                "Copy collection with sealed receipts",
            ),
            (
                Kind::Verify,
                "collection-verify",
                "Independently recheck completed copies",
            ),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (kind, capability, title))| crate::task_program::Step {
            title: title.into(),
            kind,
            capability: capability.into(),
            dependencies: if i == 0 { vec![] } else { vec![i - 1] },
            inputs: if i == 0 {
                std::collections::BTreeMap::new()
            } else {
                [(
                    "previous".into(),
                    reference(i - 1, if i == 1 { "ready" } else { "copied" }),
                )]
                .into()
            },
            when: None,
            retries: 0,
        })
        .collect();
        let program = Program { goal:"Collect the explicitly requested files and verify every copy".into(),
            constraints:vec!["Preserve originals; no overwrite; existing size, capacity and privacy limits apply".into(),"Pause takes effect after the current collection step; Stop is checked between copies".into()],
            scoped_grants:vec![format!("Native collection permission revision {} (rechecked; description grants no authority)",prepared.revision)],
            steps,completion:vec![Predicate { fact:reference(2,"verified"),equals:Value::Bool(true) }] };
        let mut adapter = Collection {
            store: self,
            policy,
            revision: prepared.revision,
            prepared: Some(prepared),
            files,
            progress: control.1,
            record: None,
        };
        task.0
            .run_program(task.1, program, control.0, &mut adapter)
            .map_err(|e| invalid(&e.detail))?;
        adapter
            .record
            .ok_or_else(|| invalid("No verified collection record"))
    }

    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    fn save(&self, record: &Record) -> io::Result<()> {
        uuid::Uuid::parse_str(&record.id).map_err(io::Error::other)?;
        fs::create_dir_all(&self.root)?;
        let path = self.root.join(format!("{}.dpapi", record.id));
        let temp = self.root.join(format!("{}.tmp", uuid::Uuid::new_v4()));
        let bytes = custodian_winsec::dpapi::protect(
            &serde_json::to_vec(record).map_err(io::Error::other)?,
        )?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(temp, path)
    }
    pub fn records(&self) -> io::Result<Vec<Record>> {
        let paths = match fs::read_dir(&self.root) {
            Ok(paths) => paths,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut records = Vec::new();
        for path in paths {
            let path = path?.path();
            if path.extension().is_none_or(|ext| ext != "dpapi") {
                continue;
            }
            let record: Record =
                serde_json::from_slice(&custodian_winsec::dpapi::unprotect(&fs::read(&path)?)?)
                    .map_err(io::Error::other)?;
            if !canonical_id(&record.id)
                || path
                    .file_stem()
                    .is_none_or(|name| name != record.id.as_str())
                || record.steps.is_empty()
                || record.steps.len() > MAX_FILES
                || record.steps.iter().any(|s| !canonical_id(&s.receipt))
                || record.steps.iter().enumerate().any(|(index, step)| {
                    record.steps[..index].iter().any(|old| {
                        old.receipt == step.receipt || old.destination == step.destination
                    })
                })
            {
                return Err(invalid("Workflow journal identity is invalid"));
            }
            record.proposal.validate(record.selection)?;
            records.push(record);
        }
        records.sort_by_key(|r| std::cmp::Reverse(r.created));
        Ok(records)
    }

    pub fn execute(
        &self,
        policy: &RoutinePolicy,
        prepared: Prepared,
        files: &FileTasks,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(&Record),
    ) -> io::Result<Record> {
        let _writer = WORKFLOWS.lock().unwrap_or_else(|e| e.into_inner());
        self.records()?;
        policy.with_collection(prepared.revision, || Ok(()))?;
        stopped(cancel)?;
        // Validate every captured object before the first mutation.
        for plan in &prepared.plans {
            plan.validate()?;
        }
        let mut record = Record {
            id: uuid::Uuid::new_v4().to_string(),
            created: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            selection: prepared.selection,
            proposal: prepared.proposal,
            state: State::Prepared,
            steps: prepared
                .plans
                .iter()
                .map(|plan| Step {
                    receipt: uuid::Uuid::new_v4().to_string(),
                    source: plan.source.clone().unwrap(),
                    destination: plan.destination.clone(),
                    verified: false,
                    undone: false,
                })
                .collect(),
            detail: "Prepared; no output created".into(),
        };
        self.save(&record)?;
        let result: io::Result<()> = (|| {
            for (index, plan) in prepared.plans.iter().enumerate() {
                stopped(cancel)?;
                record.state = State::Running;
                record.detail = format!("Copying file {} of {}", index + 1, record.steps.len());
                self.save(&record)?;
                progress(&record);
                policy.with_copy_grant(
                    CopyOperation::Collection,
                    prepared.revision,
                    std::slice::from_ref(
                        plan.source
                            .as_ref()
                            .ok_or_else(|| invalid("Missing copy source"))?,
                    ),
                    &plan.destination,
                    || {
                        // Preserve the whole collection's capacity budget before each
                        // step; each individual executor also rechecks its own budget.
                        let remaining: u64 = prepared.plans[index..]
                            .iter()
                            .map(FileTaskPlan::bytes)
                            .sum();
                        let free = custodian_winsec::fsops::available_bytes(
                            plan.destination.parent().unwrap(),
                        )?;
                        if remaining > free / 20 || free.saturating_sub(remaining) < FREE_RESERVE {
                            return Err(invalid("Collection free-space budget changed"));
                        }
                        files.execute_workflow_copy(
                            plan,
                            CopyLimits {
                                max_bytes: MAX_BACKUP_BYTES,
                                min_free_bytes: FREE_RESERVE,
                                max_space_percent: 5,
                            },
                            cancel,
                            &record.steps[index].receipt,
                        )
                    },
                )?;
                record.steps[index].verified = true;
                self.save(&record)?;
                progress(&record);
            }
            stopped(cancel)?;
            record.state = State::Verifying;
            record.detail = "Rechecking every completed copy".into();
            self.save(&record)?;
            progress(&record);
            let receipts = files.receipts()?;
            for step in &record.steps {
                stopped(cancel)?;
                let receipt = receipts
                    .iter()
                    .find(|r| r.id == step.receipt)
                    .ok_or_else(|| invalid("missing workflow receipt"))?;
                if !receipt_matches(receipt, step) {
                    return Err(invalid("Workflow receipt does not match its step"));
                }
                files.verify_copy(receipt)?;
            }
            policy.with_collection(prepared.revision, || Ok(()))?;
            #[cfg(all(test, windows))]
            crate::crash_probe::checkpoint("before-parent-completion")?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                record.state = State::Completed;
                record.detail = format!(
                    "Verified {} copies; originals preserved",
                    record.steps.len()
                );
            }
            Err(error) => {
                record.state = if error.kind() == io::ErrorKind::Interrupted {
                    State::Interrupted
                } else {
                    State::RecoveryRequired
                };
                record.detail="Stopped without a completion claim. Inspect saved receipts; existing completed copies were retained.".into();
                self.save(&record)?;
                progress(&record);
                return Err(error);
            }
        }
        self.save(&record)?;
        progress(&record);
        Ok(record)
    }

    /// Restart inspection verifies receipts only. It never replays a task.
    pub fn inspect(&self, files: &FileTasks) -> io::Result<Option<Record>> {
        let _writer = WORKFLOWS.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut record) = self.records()?.into_iter().next() else {
            return Ok(None);
        };
        let receipts = files.receipts()?;
        let mut all_verified = true;
        let mut all_undone = true;
        for step in &mut record.steps {
            crate::privacy::check(&step.source)?;
            crate::privacy::check(&step.destination)?;
            match receipts.iter().find(|r| r.id == step.receipt) {
                Some(receipt) if receipt.state == ReceiptState::Completed => {
                    step.verified =
                        receipt_matches(receipt, step) && files.verify_copy(receipt).is_ok();
                    step.undone = false;
                    all_undone = false;
                    all_verified &= step.verified;
                }
                Some(receipt)
                    if receipt.state == ReceiptState::Undone && receipt_matches(receipt, step) =>
                {
                    step.undone = true;
                    step.verified = false;
                    all_verified = false;
                }
                None if step.undone && !step.destination.try_exists()? => {
                    step.verified = false;
                    all_verified = false;
                }
                Some(receipt)
                    if receipt.state == ReceiptState::Failed
                        && receipt_matches(receipt, step)
                        && step.undone
                        && !step.destination.try_exists()? =>
                {
                    step.verified = false;
                    all_verified = false;
                }
                _ => {
                    step.verified = false;
                    step.undone = false;
                    all_verified = false;
                    all_undone = false;
                }
            }
        }
        if all_undone {
            record.state = State::Undone;
        } else if !all_verified || record.state != State::Completed {
            record.state = State::RecoveryRequired;
        }
        record.detail = format!(
            "{} of {} copies currently verified; {} undone. No steps replayed.",
            record.steps.iter().filter(|s| s.verified).count(),
            record.steps.len(),
            record.steps.iter().filter(|s| s.undone).count()
        );
        Ok(Some(record))
    }

    /// Explicit native user action. Receipt IDs were saved before execution,
    /// including the crash window after a copy but before step publication.
    pub fn undo_latest(&self, files: &FileTasks, cancel: &AtomicBool) -> io::Result<Record> {
        let _writer = WORKFLOWS.lock().unwrap_or_else(|e| e.into_inner());
        let mut record = self
            .records()?
            .into_iter()
            .next()
            .ok_or_else(|| invalid("No saved workflow"))?;
        for step in &record.steps {
            crate::privacy::check(&step.source)?;
            crate::privacy::check(&step.destination)?;
        }
        record.state = State::Undoing;
        record.detail = "Undo requested; verifying each copy before removal".into();
        self.save(&record)?;
        let mut blocked = false;
        let mut removed = 0;
        for index in (0..record.steps.len()).rev() {
            stopped(cancel)?;
            let receipts = files.receipts()?;
            if let Some(receipt) = receipts
                .iter()
                .find(|r| r.id == record.steps[index].receipt)
            {
                if !receipt_matches(receipt, &record.steps[index]) {
                    return Err(invalid("Workflow receipt does not match its step"));
                }
                match receipt.state {
                    ReceiptState::Completed => match files.undo(&receipt.id) {
                        Ok(_) => {
                            removed += 1;
                            record.steps[index].undone = true;
                            record.steps[index].verified = false;
                        }
                        Err(_) => {
                            blocked = true;
                            record.steps[index].verified = false;
                        }
                    },
                    ReceiptState::Undone => {
                        record.steps[index].undone = true;
                        record.steps[index].verified = false;
                    }
                    ReceiptState::Prepared | ReceiptState::RecoveryRequired => {
                        blocked = true;
                    }
                    ReceiptState::Failed => {
                        if record.steps[index].destination.try_exists()? {
                            blocked = true;
                        } else {
                            record.steps[index].undone = true;
                        }
                    }
                }
            } else if record.steps[index].destination.try_exists()? {
                blocked = true;
            } else {
                record.steps[index].undone = true;
                record.steps[index].verified = false;
            }
            self.save(&record)?;
        }
        if blocked {
            record.state = State::RecoveryRequired;
            record.detail=format!("{removed} verified copies removed. Changed, partial or unverified outputs remain for review.");
            self.save(&record)?;
            return Err(invalid(&record.detail));
        }
        record.state = State::Undone;
        record.detail="Verified completed copies removed; originals preserved. Unstarted steps were not replayed.".into();
        self.save(&record)?;
        Ok(record)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn coordinated_real_collection_keeps_receipts_verification_and_undo() {
        use crate::coordinator::{TaskCoordinator, TaskState};
        let (_root, goal, policy, files, store, cancel) = fixture();
        let prepared = prepared(&policy, &goal, &cancel).unwrap();
        let coordinator = TaskCoordinator::new();
        let (id, cancel) = coordinator
            .create_task(
                "Real collection".into(),
                TaskState::Running,
                std::slice::from_ref(&goal.destination),
            )
            .unwrap();
        let record = store
            .execute_coordinated(
                (&coordinator, &id),
                &policy,
                prepared,
                &files,
                (&cancel, &mut |_| {}),
            )
            .unwrap();
        let task = coordinator.get_task(&id).unwrap();
        assert_eq!(task.state, TaskState::Completed);
        let checkpoint = task.execution.unwrap();
        assert!(checkpoint.verified);
        assert_eq!(checkpoint.outputs.len(), 3);
        assert_eq!(checkpoint.outputs[&1].receipts.len(), record.steps.len());
        assert_eq!(
            store.inspect(&files).unwrap().unwrap().state,
            State::Completed
        );
        store.undo_latest(&files, &cancel).unwrap();
        for step in record.steps {
            assert!(step.source.is_file());
            assert!(!step.destination.exists());
        }
    }
    #[test]
    fn coordinated_preflight_refuses_stale_files_without_starting_copy() {
        use crate::coordinator::{TaskCoordinator, TaskState};
        let (_root, goal, policy, files, store, cancel) = fixture();
        let prepared = prepared(&policy, &goal, &cancel).unwrap();
        fs::write(goal.roots[0].join("a.txt"), b"changed").unwrap();
        let coordinator = TaskCoordinator::new();
        let (id, cancel) = coordinator
            .create_task("Stale collection".into(), TaskState::Running, &[])
            .unwrap();
        assert!(store
            .execute_coordinated(
                (&coordinator, &id),
                &policy,
                prepared,
                &files,
                (&cancel, &mut |_| {})
            )
            .is_err());
        assert!(files.receipts().unwrap().is_empty());
        assert_eq!(fs::read_dir(&goal.destination).unwrap().count(), 0);
        assert_eq!(coordinator.get_task(&id).unwrap().state, TaskState::Failed);
    }
    #[test]
    fn same_coordinator_composes_real_files_and_explicitly_mock_app_adapter() {
        use crate::{
            coordinator::{TaskCoordinator, TaskState},
            task_program as p,
        };
        struct Mixed<'a> {
            policy: &'a RoutinePolicy,
            files: &'a FileTasks,
            store: &'a WorkflowStore,
            prepared: Option<Prepared>,
            mock_calls: usize,
        }
        impl p::Adapter for Mixed<'_> {
            fn authorize(&mut self, step: &p::Step) -> Result<(), p::Failure> {
                if !matches!(
                    (step.kind, step.capability.as_str()),
                    (p::Kind::Write, "collection-copy")
                        | (p::Kind::App, "mock.app.acknowledge")
                        | (p::Kind::Verify, "collection-verify")
                ) {
                    return Err(p::Failure::new(
                        p::FailureClass::UnavailableCapability,
                        "Unregistered fixture adapter",
                    ));
                }
                self.policy
                    .with_collection(self.policy.revision(), || Ok(()))
                    .map_err(|e| p::Failure::new(p::FailureClass::MissingAuthority, e.to_string()))
            }
            fn execute(
                &mut self,
                step: &p::Step,
                inputs: &p::Facts,
                cancel: &AtomicBool,
            ) -> Result<p::Outcome, p::Failure> {
                let outcome: io::Result<p::Outcome> = (|| match step.capability.as_str() {
                    "collection-copy" => {
                        let record = self.store.execute(
                            self.policy,
                            self.prepared.take().unwrap(),
                            self.files,
                            cancel,
                            &mut |_| {},
                        )?;
                        Ok(p::Outcome {
                            facts: [("ok".into(), p::Value::Bool(true))].into(),
                            receipts: record.steps.iter().map(|s| s.receipt.clone()).collect(),
                        })
                    }
                    "mock.app.acknowledge" => {
                        assert_eq!(inputs.get("previous"), Some(&p::Value::Bool(true)));
                        self.mock_calls += 1;
                        Ok(p::Outcome {
                            facts: [("ok".into(), p::Value::Bool(true))].into(),
                            receipts: vec![
                                "mock.app.receipt (no real application controlled)".into()
                            ],
                        })
                    }
                    "collection-verify" => {
                        let record = self.store.inspect(self.files)?.unwrap();
                        if record.state != State::Completed {
                            return Err(invalid("Changed outputs"));
                        }
                        Ok(p::Outcome {
                            facts: [("ok".into(), p::Value::Bool(self.mock_calls == 1))].into(),
                            receipts: vec![record.id],
                        })
                    }
                    _ => Err(invalid("Unavailable fixture capability")),
                })();
                outcome.map_err(|e| p::Failure::new(p::FailureClass::StaleWorld, e.to_string()))
            }
        }
        let (_root, goal, policy, files, store, cancel) = fixture();
        let prepared = prepared(&policy, &goal, &cancel).unwrap();
        let coordinator = TaskCoordinator::new();
        let (id, cancel) = coordinator
            .create_task(
                "Real files + mock app fixture".into(),
                TaskState::Running,
                std::slice::from_ref(&goal.destination),
            )
            .unwrap();
        let steps = [
            (p::Kind::Write, "collection-copy"),
            (p::Kind::App, "mock.app.acknowledge"),
            (p::Kind::Verify, "collection-verify"),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (kind, capability))| p::Step {
            title: capability.into(),
            kind,
            capability: capability.into(),
            dependencies: if i == 0 { vec![] } else { vec![i - 1] },
            inputs: if i == 0 {
                std::collections::BTreeMap::new()
            } else {
                [(
                    "previous".into(),
                    p::Reference {
                        step: i - 1,
                        key: "ok".into(),
                    },
                )]
                .into()
            },
            when: None,
            retries: 0,
        })
        .collect();
        let program = p::Program {
            goal: "Real receipt-backed files followed by a MOCK application acknowledgment".into(),
            constraints: vec![],
            scoped_grants: vec![],
            steps,
            completion: vec![p::Predicate {
                fact: p::Reference {
                    step: 2,
                    key: "ok".into(),
                },
                equals: p::Value::Bool(true),
            }],
        };
        let mut adapter = Mixed {
            policy: &policy,
            files: &files,
            store: &store,
            prepared: Some(prepared),
            mock_calls: 0,
        };
        let result = coordinator
            .run_program(&id, program, &cancel, &mut adapter)
            .unwrap();
        assert!(result.verified);
        assert_eq!(adapter.mock_calls, 1);
        assert!(result.outputs[&1].receipts[0].contains("no real application"));
        store.undo_latest(&files, &cancel).unwrap();
        assert!(goal.roots[0].join("a.txt").is_file());
        assert_eq!(fs::read_dir(&goal.destination).unwrap().count(), 0);
    }
    fn fixture() -> (
        tempfile::TempDir,
        Goal,
        RoutinePolicy,
        FileTasks,
        WorkflowStore,
        AtomicBool,
    ) {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(source.join("a.txt"), b"document one").unwrap();
        fs::write(source.join("b.pdf"), b"bigger document two").unwrap();
        fs::write(source.join("photo.jpg"), b"photo").unwrap();
        let policy = RoutinePolicy::default();
        policy.update_permissions(0, false, Some(true)).unwrap();
        let files = FileTasks::new(root.path().join("files"));
        let store = WorkflowStore::new(root.path().join("workflows"));
        (
            root,
            Goal {
                roots: vec![source],
                destination,
                selection: Selection {
                    filter: Filter::Documents,
                    largest: false,
                },
            },
            policy,
            files,
            store,
            AtomicBool::new(false),
        )
    }
    fn prepared(policy: &RoutinePolicy, goal: &Goal, cancel: &AtomicBool) -> io::Result<Prepared> {
        prepare(
            policy,
            policy.revision(),
            goal,
            Proposal {
                steps: goal.selection.steps(),
                attempts: 1,
            },
            cancel,
            &|_| true,
            &mut |_| {},
        )
    }
    #[test]
    fn shared_compositions_execute_verify_and_group_undo_after_restart() {
        for selection in [
            Selection {
                filter: Filter::Any,
                largest: true,
            },
            Selection {
                filter: Filter::Images,
                largest: true,
            },
            Selection {
                filter: Filter::Documents,
                largest: false,
            },
            Selection {
                filter: Filter::Images,
                largest: false,
            },
            Selection {
                filter: Filter::Documents,
                largest: true,
            },
        ] {
            let (root, mut goal, policy, files, store, cancel) = fixture();
            goal.selection = selection;
            let count = if selection.filter == Filter::Documents && !selection.largest {
                2
            } else {
                1
            };
            let result = store
                .execute(
                    &policy,
                    prepared(&policy, &goal, &cancel).unwrap(),
                    &files,
                    &cancel,
                    &mut |_| {},
                )
                .unwrap();
            assert_eq!(result.state, State::Completed);
            assert_eq!(result.steps.len(), count);
            assert!(result
                .steps
                .iter()
                .all(|s| s.verified && s.source.exists() && s.destination.exists()));
            let restarted = WorkflowStore::new(root.path().join("workflows"));
            assert_eq!(
                restarted.inspect(&files).unwrap().unwrap().state,
                State::Completed
            );
            let undone = restarted.undo_latest(&files, &cancel).unwrap();
            assert_eq!(undone.state, State::Undone);
            assert!(undone
                .steps
                .iter()
                .all(|s| s.source.exists() && !s.destination.exists()));
            assert_eq!(
                restarted.inspect(&files).unwrap().unwrap().state,
                State::Undone
            );
        }
    }
    #[test]
    fn wrong_model_steps_or_extra_authority_are_rejected_before_discovery() {
        let (_root, goal, policy, _files, store, cancel) = fixture();
        for steps in [
            vec!["discover", "select_all", "copy", "verify"],
            vec![
                "discover",
                "filter_documents",
                "select_all",
                "delete",
                "verify",
            ],
            vec![
                "discover",
                "filter_documents",
                "select_all",
                "verify",
                "copy",
            ],
        ] {
            assert!(prepare(
                &policy,
                policy.revision(),
                &goal,
                Proposal {
                    steps: steps.into_iter().map(str::to_owned).collect(),
                    attempts: 1,
                },
                &cancel,
                &|_| true,
                &mut |_| panic!("must not scan")
            )
            .is_err());
        }
        assert!(serde_json::from_str::<Proposal>(
            r#"{"steps":["discover","select_all","copy","verify"],"grant":true}"#
        )
        .is_err());
        assert!(store.records().unwrap().is_empty());
        assert_eq!(fs::read_dir(&goal.destination).unwrap().count(), 0);
    }

    #[test]
    fn complete_collection_size_is_checked_before_hashing_or_creating_outputs() {
        let (_root, goal, policy, _files, store, cancel) = fixture();
        fs::File::create(goal.roots[0].join("large.pdf"))
            .unwrap()
            .set_len(MAX_BACKUP_BYTES + 1)
            .unwrap();
        assert!(prepared(&policy, &goal, &cancel).is_err());
        assert_eq!(fs::read_dir(&goal.destination).unwrap().count(), 0);
        assert!(store.records().unwrap().is_empty());
    }
    #[test]
    fn conflicts_duplicate_names_limits_and_revoked_grants_make_no_copy() {
        let (_root, goal, policy, files, store, cancel) = fixture();
        fs::write(goal.destination.join("b.pdf"), b"existing").unwrap();
        assert!(prepared(&policy, &goal, &cancel).is_err());
        assert!(!goal.destination.join("a.txt").exists());
        fs::remove_file(goal.destination.join("b.pdf")).unwrap();
        let nested = goal.roots[0].join("nested");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("a.txt"), b"duplicate").unwrap();
        assert!(prepared(&policy, &goal, &cancel).is_err());
        fs::remove_file(nested.join("a.txt")).unwrap();
        let plan = prepared(&policy, &goal, &cancel).unwrap();
        policy.update_permissions(1, false, Some(false)).unwrap();
        policy.update_permissions(2, false, Some(true)).unwrap();
        assert!(store
            .execute(&policy, plan, &files, &cancel, &mut |_| {})
            .is_err());
        assert_eq!(fs::read_dir(&goal.destination).unwrap().count(), 0);
        for n in 0..MAX_FILES {
            fs::write(nested.join(format!("{n}.txt")), b"x").unwrap();
        }
        assert!(prepared(&policy, &goal, &cancel).is_err());
        assert!(store.records().unwrap().is_empty());
    }
    #[test]
    fn private_files_and_nested_destination_are_excluded_and_original_permission_does_not_expand() {
        let (_root, goal, policy, files, store, cancel) = fixture();
        let private = goal.roots[0].join("private");
        fs::create_dir(&private).unwrap();
        fs::write(private.join("canary.txt"), vec![0; 4096]).unwrap();
        let privacy = Arc::new(crate::privacy::AccessPolicy::default());
        privacy
            .replace_private_paths(vec![private.clone()])
            .unwrap();
        crate::privacy::with_policy(privacy, || {
            let plan = prepared(&policy, &goal, &cancel).unwrap();
            assert_eq!(plan.plans.len(), 2);
            assert!(plan.excluded > 0);
            store
                .execute(&policy, plan, &files, &cancel, &mut |_| {})
                .unwrap();
            assert!(!goal.destination.join("canary.txt").exists());
        });
        let other = RoutinePolicy::default();
        other.update(0, true).unwrap();
        assert!(prepared(&other, &goal, &cancel).is_err());
        let nested = goal.roots[0].join("out");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("prior.jpg"), b"previous output").unwrap();
        let othergoal = Goal {
            roots: goal.roots.clone(),
            destination: nested,
            selection: Selection {
                filter: Filter::Images,
                largest: false,
            },
        };
        assert_eq!(
            prepared(&policy, &othergoal, &cancel).unwrap().plans.len(),
            1
        );
    }
    #[test]
    fn interruption_and_completion_publication_gap_are_recovered_without_replay() {
        let (root, goal, policy, files, store, cancel) = fixture();
        let plan = prepared(&policy, &goal, &cancel).unwrap();
        let result = store.execute(&policy, plan, &files, &cancel, &mut |r| {
            if r.steps[0].verified {
                cancel.store(true, Ordering::Relaxed);
            }
        });
        assert!(result.is_err());
        assert!(goal.destination.join("a.txt").exists());
        assert!(!goal.destination.join("b.pdf").exists());
        // Real sealed on-disk state for the crash gap: file receipt completed,
        // parent workflow still says Running/unverified. No mutation is replayed.
        let mut saved = store.records().unwrap().remove(0);
        saved.state = State::Running;
        saved.steps[0].verified = false;
        store.save(&saved).unwrap();
        let restarted = WorkflowStore::new(root.path().join("workflows"));
        let inspected = restarted.inspect(&files).unwrap().unwrap();
        assert_eq!(inspected.state, State::RecoveryRequired);
        assert!(inspected.steps[0].verified);
        assert!(!goal.destination.join("b.pdf").exists());
        cancel.store(false, Ordering::Relaxed);
        let undone = restarted.undo_latest(&files, &cancel).unwrap();
        assert_eq!(undone.state, State::Undone);
        assert!(!goal.destination.join("a.txt").exists());
        assert!(goal.roots[0].join("a.txt").exists());
        assert_eq!(
            restarted.inspect(&files).unwrap().unwrap().state,
            State::Undone
        );
    }
    #[test]
    fn changed_output_is_retained_while_other_verified_copies_can_be_undone() {
        let (_root, goal, policy, files, store, cancel) = fixture();
        store
            .execute(
                &policy,
                prepared(&policy, &goal, &cancel).unwrap(),
                &files,
                &cancel,
                &mut |_| {},
            )
            .unwrap();
        fs::write(goal.destination.join("b.pdf"), b"user edited after copy").unwrap();
        assert_eq!(
            store.inspect(&files).unwrap().unwrap().state,
            State::RecoveryRequired
        );
        assert!(store.undo_latest(&files, &cancel).is_err());
        assert_eq!(
            fs::read(goal.destination.join("b.pdf")).unwrap(),
            b"user edited after copy"
        );
        assert!(!goal.destination.join("a.txt").exists());
        assert!(goal.roots[0].join("a.txt").exists());
    }
    #[test]
    fn deterministic_receipt_refuses_replay_and_tampered_workflow_blocks_new_writes() {
        let (_root, goal, policy, files, store, cancel) = fixture();
        let plan = prepared(&policy, &goal, &cancel).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let limits = CopyLimits {
            max_bytes: MAX_BACKUP_BYTES,
            min_free_bytes: FREE_RESERVE,
            max_space_percent: 5,
        };
        files
            .execute_workflow_copy(&plan.plans[0], limits, &cancel, &id)
            .unwrap();
        files.undo(&id).unwrap();
        assert!(files
            .execute_workflow_copy(&plan.plans[0], limits, &cancel, &id)
            .is_err());
        assert!(!goal.destination.join("a.txt").exists());
        fs::create_dir(&store.root).unwrap();
        fs::write(
            store.root.join(format!("{}.dpapi", uuid::Uuid::new_v4())),
            b"corrupt",
        )
        .unwrap();
        assert!(store
            .execute(&policy, plan, &files, &cancel, &mut |_| {})
            .is_err());
        assert!(!goal.destination.join("b.pdf").exists());
    }
}
