//! Native approved application goals. Apps edit a task-owned copy; publication
//! uses the existing verified file executor and retains its safe undo receipt.
#[cfg(windows)]
pub use native::{execute_after_native_consent, ApplicationGoal, ApplicationPlan, Programs};
#[cfg(windows)]
mod native {
    use crate::{
        app_adapter::{
            execute_checked, validate_cell_edits, ApplicationAction, ApplicationExecutor, CellEdit,
        },
        app_authority::{AppPermission, ApplicationAuthority},
        app_journal::{ApplicationJournal, Event},
        coordinator::TaskCoordinator,
        editor_session::{EditorProgram, EditorSession},
        excel_session::{ExcelProgram, ExcelSession},
        file_tasks::{CopyLimits, FileAction, FileReceipt, FileTaskPlan, FileTasks},
        task_program::{
            Adapter, Checkpoint, Facts, Failure, FailureClass, Kind, Outcome, Predicate, Program,
            Reference, Step, Value,
        },
    };
    use serde::Serialize;
    use std::{
        collections::BTreeMap,
        fs::{self, File},
        io::Read,
        path::{Path, PathBuf},
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };
    fn failure(detail: impl Into<String>) -> Failure {
        Failure::new(FailureClass::Unrecoverable, detail)
    }
    fn io_failure(_: std::io::Error) -> Failure {
        failure("Application file state or journal changed; inspect recovery evidence")
    }
    #[derive(Debug, Clone, Serialize)]
    pub enum ApplicationGoal {
        Text { find: String, replacement: String },
        Cells { edits: Vec<CellEdit> },
    }
    pub struct ApplicationPlan {
        pub source: PathBuf,
        pub destination: PathBuf,
        pub workspace: PathBuf,
        pub document_id: String,
        pub executable: String,
        goal: ApplicationGoal,
        preflight: FileTaskPlan,
        revision: u64,
        source_digest: [u8; 32],
        source_bytes: u64,
        expected: crate::app_journal::ExpectedArtifact,
        range: Option<(u64, u64)>,
        digest: [u8; 32],
        file_revision: u64,
        source_identity: custodian_winsec::fsops::FileIdentity,
    }
    impl ApplicationPlan {
        /// Preparation has no app effects or filesystem writes. The caller
        /// retains this native object behind the user's one-use approval card.
        pub fn prepare(
            source: &Path,
            destination: &Path,
            goal: ApplicationGoal,
            executable: &str,
            authority: &ApplicationAuthority,
        ) -> Result<Self, Failure> {
            crate::privacy::check(source).map_err(io_failure)?;
            crate::privacy::check(destination).map_err(io_failure)?;
            let _read_guard = custodian_winsec::guarded_copy::CopyGuard::open(source, destination)
                .map_err(io_failure)?;
            let identity = custodian_winsec::fsops::file_identity(source).map_err(io_failure)?;
            let document_id = format!("file-document:{}:{}", identity.volume, identity.index);
            let revision = authority.admit_document_source(executable, &document_id)?;
            let limit = if matches!(goal, ApplicationGoal::Text { .. }) {
                65536
            } else {
                8 * 1024 * 1024
            };
            if fs::metadata(source).map_err(io_failure)?.len() > limit {
                return Err(failure("Source exceeds the qualified application limit"));
            }
            let preflight = FileTaskPlan::prepare(FileAction::Copy, Some(source), destination)
                .map_err(io_failure)?;
            let source = preflight
                .source
                .clone()
                .ok_or_else(|| failure("Source file required"))?;
            let destination = preflight.destination.clone();
            let mut bytes = vec![];
            File::open(&source)
                .map_err(io_failure)?
                .take(limit + 1)
                .read_to_end(&mut bytes)
                .map_err(io_failure)?;
            if bytes.len() as u64 > limit {
                return Err(failure("Source exceeds the qualified application limit"));
            }
            let (expected, range) = match &goal {
                ApplicationGoal::Text { find, replacement } => {
                    if source.extension().and_then(|s| s.to_str()) != Some("txt")
                        || destination.extension().and_then(|s| s.to_str()) != Some("txt")
                        || find.is_empty()
                        || find.len() > 65536
                        || replacement.len() > 65536
                        || replacement
                            .chars()
                            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
                    {
                        return Err(failure("Use bounded UTF-8 .txt files and an exact nonempty replacement selection"));
                    }
                    let original = std::str::from_utf8(&bytes)
                        .map_err(|_| failure("Only UTF-8 text is qualified"))?;
                    if original.starts_with('\u{feff}') || original.contains('\0') {
                        return Err(failure("This text encoding or content is unqualified"));
                    }
                    let mut occurrences = original.match_indices(find);
                    let start = occurrences
                        .next()
                        .ok_or_else(|| failure("The selected text was not found"))?
                        .0;
                    if occurrences.next().is_some() {
                        return Err(failure(
                            "The selected text is ambiguous; choose a unique range",
                        ));
                    }
                    let end = start + find.len();
                    let mut expected = original.to_string();
                    expected.replace_range(start..end, replacement);
                    if expected.len() > 65536 {
                        return Err(failure("Edited text exceeds its limit"));
                    }
                    (
                        crate::app_journal::ExpectedArtifact::Text {
                            digest: *blake3::hash(expected.as_bytes()).as_bytes(),
                            bytes: expected.len() as u64,
                        },
                        Some((start as u64, end as u64)),
                    )
                }
                ApplicationGoal::Cells { edits } => {
                    if source.extension().and_then(|s| s.to_str()) != Some("xlsx")
                        || destination.extension().and_then(|s| s.to_str()) != Some("xlsx")
                    {
                        return Err(failure("Only qualified .xlsx workbooks are supported"));
                    }
                    validate_cell_edits("native-sheet-pending", edits).map_err(failure)?;
                    let snapshot = crate::workbook::inspect(&bytes).map_err(failure)?;
                    if crate::workbook::recalculate(&snapshot).map_err(failure)? != snapshot {
                        return Err(failure("Workbook calculation cache is stale"));
                    }
                    let expected =
                        crate::workbook::apply_edits(&snapshot, edits).map_err(failure)?;
                    (
                        crate::app_journal::ExpectedArtifact::Workbook {
                            snapshot_digest: *blake3::hash(
                                &serde_json::to_vec(&expected)
                                    .map_err(|_| failure("Workbook prediction failed"))?,
                            )
                            .as_bytes(),
                        },
                        None,
                    )
                }
            };
            preflight.validate().map_err(io_failure)?;
            let workspace = destination
                .parent()
                .ok_or_else(|| failure("Destination parent required"))?
                .join(format!(".custodian-app-{}", uuid::Uuid::new_v4()));
            let source_digest = *blake3::hash(&bytes).as_bytes();
            let digest = *blake3::hash(
                &serde_json::to_vec(&(
                    &source,
                    &destination,
                    &workspace,
                    &goal,
                    source_digest,
                    revision,
                ))
                .map_err(|_| failure("Goal serialization failed"))?,
            )
            .as_bytes();
            Ok(Self {
                source,
                destination,
                workspace,
                document_id,
                executable: executable.into(),
                goal,
                preflight,
                revision,
                source_digest,
                source_bytes: bytes.len() as u64,
                expected,
                range,
                digest,
                file_revision: crate::privacy::current().revision(),
                source_identity: identity,
            })
        }
        pub fn validate(&self, authority: &ApplicationAuthority) -> Result<(), Failure> {
            self.preflight.validate().map_err(io_failure)?;
            if authority.admit_document_source(&self.executable, &self.document_id)?
                != self.revision
            {
                return Err(Failure::new(
                    FailureClass::MissingAuthority,
                    "Application privacy changed since approval",
                ));
            }
            Ok(())
        }
    }
    pub enum Programs<'a> {
        Editor(&'a EditorProgram),
        Excel(&'a ExcelProgram),
    }
    enum Session<'a> {
        Editor(EditorSession<'a>),
        Excel(ExcelSession<'a>),
    }
    impl Session<'_> {
        fn request(&self, action: ApplicationAction) -> crate::app_adapter::ApplicationRequest {
            match self {
                Self::Editor(s) => s.request(action),
                Self::Excel(s) => s.request(action),
            }
        }
        fn executor(&mut self) -> &mut dyn ApplicationExecutor {
            match self {
                Self::Editor(s) => s,
                Self::Excel(s) => s,
            }
        }
    }
    fn program(cells: bool) -> Program {
        let keys = [
            "app.open_bound_document",
            "app.inspect_bound_document",
            if cells {
                "app.edit_bound_cells"
            } else {
                "app.edit_bound_range"
            },
            "app.save_bound_document",
            "app.verify_saved_output",
            "app.close_document_normally",
            "file.publish_verified_app_output",
            "file.verify_app_publication",
        ];
        let steps = keys
            .iter()
            .enumerate()
            .map(|(i, key)| Step {
                title: (*key).replace(['.', '_'], " "),
                kind: match i {
                    1 => Kind::Read,
                    4 | 7 => Kind::Verify,
                    6 => Kind::Write,
                    _ => Kind::App,
                },
                capability: (*key).into(),
                dependencies: if i == 0 { vec![] } else { vec![i - 1] },
                inputs: BTreeMap::new(),
                when: None,
                retries: 0,
            })
            .collect();
        Program {
            goal: "Edit the bound application copy and verify its published output".into(),
            constraints: vec![
                "Preserve the original and unrelated work; never overwrite a destination".into(),
            ],
            scoped_grants: vec![
                "Native one-task consent; descriptive text grants no authority".into(),
            ],
            steps,
            completion: vec![Predicate {
                fact: Reference {
                    step: 7,
                    key: "verified".into(),
                },
                equals: Value::Bool(true),
            }],
        }
    }
    struct Runner<'a> {
        plan: &'a ApplicationPlan,
        authority: &'a ApplicationAuthority,
        session: Session<'a>,
        journal: ApplicationJournal,
        files: &'a FileTasks,
        stage: PathBuf,
        index: usize,
        receipt: Option<FileReceipt>,
        closed: bool,
    }
    impl Adapter for Runner<'_> {
        fn authorize(&mut self, step: &Step) -> Result<(), Failure> {
            // Index and capability must match the native frozen program. No
            // string supplied by a model selects an application operation.
            let expected = program(matches!(self.plan.goal, ApplicationGoal::Cells { .. }));
            if expected.steps.get(self.index) != Some(step) {
                return Err(failure("Unexpected application program step"));
            }
            if self.index < 7 {
                self.plan.validate(self.authority)
            } else {
                crate::privacy::check(&self.plan.source).map_err(io_failure)?;
                crate::privacy::check(&self.plan.destination).map_err(io_failure)?;
                if self
                    .authority
                    .admit_document_source(&self.plan.executable, &self.plan.document_id)?
                    != self.plan.revision
                    || crate::privacy::current().revision() != self.plan.file_revision
                    || custodian_winsec::fsops::file_identity(&self.plan.source)
                        .map_err(io_failure)?
                        != self.plan.source_identity
                {
                    return Err(failure(
                        "Application or file authority changed before completion",
                    ));
                }
                Ok(())
            }
        }
        fn execute(
            &mut self,
            _: &Step,
            _: &Facts,
            cancel: &AtomicBool,
        ) -> Result<Outcome, Failure> {
            if cancel.load(Ordering::Acquire) {
                return Err(failure("Application task stopped"));
            }
            let mut receipts = vec![];
            if self.index < 6 {
                let (action, permission) = match self.index {
                    0 => (ApplicationAction::OpenBoundDocument, AppPermission::Open),
                    1 => (
                        ApplicationAction::InspectBoundDocument,
                        AppPermission::Inspect,
                    ),
                    2 => (
                        match &self.plan.goal {
                            ApplicationGoal::Text { replacement, .. } => {
                                let (start, end) = self
                                    .plan
                                    .range
                                    .ok_or_else(|| failure("Native range missing"))?;
                                ApplicationAction::EditBoundRange {
                                    start,
                                    end,
                                    replacement: replacement.clone(),
                                }
                            }
                            ApplicationGoal::Cells { edits } => match &self.session {
                                Session::Excel(s) => ApplicationAction::EditBoundCells {
                                    sheet_id: s.sheet_id().into(),
                                    cells: edits.clone(),
                                },
                                _ => {
                                    return Err(failure(
                                        "Workbook goal requires the qualified Excel adapter",
                                    ))
                                }
                            },
                        },
                        AppPermission::Edit,
                    ),
                    3 => (ApplicationAction::SaveBoundDocument, AppPermission::Save),
                    4 => (ApplicationAction::VerifySavedOutput, AppPermission::Verify),
                    5 => (
                        ApplicationAction::CloseDocumentNormally,
                        AppPermission::Close,
                    ),
                    _ => return Err(failure("Invalid application index")),
                };
                let request = self.session.request(action);
                let grant = self
                    .authority
                    .issue_after_native_consent(&request, &[permission], Duration::from_secs(60))
                    .map_err(failure)?;
                let receipt = match execute_checked(
                    &request,
                    self.authority,
                    &grant,
                    cancel,
                    self.session.executor(),
                ) {
                    Ok(receipt) => receipt,
                    Err(error) => {
                        if !matches!(self.index, 1 | 4) {
                            if let Some(receipt) = error.recovery_receipt {
                                self.journal
                                    .record_receipt(&request, *receipt)
                                    .map_err(io_failure)?;
                            }
                        }
                        return Err(failure(error.detail));
                    }
                };
                if !matches!(self.index, 1 | 4) {
                    self.journal
                        .record_receipt(&request, receipt)
                        .map_err(io_failure)?;
                    receipts.push(format!(
                        "app-receipt:{}",
                        blake3::hash(
                            &serde_json::to_vec(&request)
                                .map_err(|_| failure("Receipt digest failed"))?
                        )
                    ));
                }
                if self.index == 5 {
                    self.closed = true;
                }
            } else if self.index == 6 {
                if !self.closed {
                    return Err(failure("Normal application close has not been verified"));
                }
                let _stage_pin = custodian_winsec::guarded_copy::CopyGuard::open(
                    &self.stage,
                    &self.plan.destination,
                )
                .map_err(io_failure)?;
                let mut bytes = vec![];
                File::open(&self.stage)
                    .map_err(io_failure)?
                    .take(8 * 1024 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(io_failure)?;
                if bytes.len() > 8 * 1024 * 1024 {
                    return Err(failure("Application output exceeds its limit"));
                }
                if !self.plan.expected.matches(&bytes) {
                    return Err(failure(
                        "Saved application output differs from the approved result",
                    ));
                }
                if matches!(self.plan.goal, ApplicationGoal::Cells { .. }) {
                    let actual = crate::workbook::inspect(&bytes).map_err(failure)?;
                    if crate::workbook::recalculate(&actual).map_err(failure)? != actual {
                        return Err(failure("Saved workbook calculation differs"));
                    }
                }
                self.journal
                    .append(Event::OutputVerified {
                        digest: *blake3::hash(&bytes).as_bytes(),
                        bytes: bytes.len() as u64,
                    })
                    .map_err(io_failure)?;
                let plan = FileTaskPlan::prepare(
                    FileAction::Copy,
                    Some(&self.stage),
                    &self.plan.destination,
                )
                .map_err(io_failure)?;
                let id = uuid::Uuid::new_v4().to_string();
                self.journal
                    .append(Event::PublishPending {
                        receipt_id: id.clone(),
                    })
                    .map_err(io_failure)?;
                let receipt = self
                    .files
                    .execute_workflow_copy(
                        &plan,
                        CopyLimits {
                            max_bytes: 8 * 1024 * 1024,
                            min_free_bytes: crate::routine::FREE_RESERVE,
                            max_space_percent: 10,
                        },
                        cancel,
                        &id,
                    )
                    .map_err(io_failure)?;
                self.journal
                    .append(Event::Published {
                        receipt_id: id.clone(),
                    })
                    .map_err(io_failure)?;
                receipts.push(id);
                self.receipt = Some(receipt);
            } else if self.index == 7 {
                let receipt = self
                    .receipt
                    .as_ref()
                    .ok_or_else(|| failure("Publication receipt missing"))?;
                self.files.verify_copy(receipt).map_err(io_failure)?;
                // The output now exists, so preflight.validate's no-existing-
                // destination test is intentionally replaced by receipt identity.
                crate::privacy::check(&self.plan.source).map_err(io_failure)?;
                if crate::hashing::hash_file(&self.plan.source).map_err(io_failure)?
                    != blake3::Hash::from_bytes(self.plan.source_digest)
                        .to_hex()
                        .as_str()
                {
                    return Err(failure(
                        "Original source changed; keep the output for recovery review",
                    ));
                }
                self.journal.append(Event::Completed).map_err(io_failure)?;
            } else {
                return Err(failure("Application program exhausted"));
            }
            self.index += 1;
            Ok(Outcome {
                facts: BTreeMap::from([("verified".into(), Value::Bool(true))]),
                receipts,
            })
        }
    }
    /// Invoke only for a stored native plan accepted by the person. Applications
    /// receive fresh per-step grants internally; there is no per-step prompt.
    #[allow(clippy::too_many_arguments)]
    pub fn execute_after_native_consent(
        plan: &ApplicationPlan,
        programs: Programs<'_>,
        authority: &ApplicationAuthority,
        coordinator: &TaskCoordinator,
        task_id: &str,
        files: &FileTasks,
        cancel: &AtomicBool,
    ) -> Result<Checkpoint, Failure> {
        plan.validate(authority)?;
        if cancel.load(Ordering::Acquire) {
            return Err(failure("Application task stopped"));
        }
        coordinator
            .update_state(
                task_id,
                crate::coordinator::TaskState::Running,
                Some("Preparing the approved application workspace".into()),
            )
            .map_err(|e| failure(e.to_string()))?;
        coordinator
            .reserve_resources(
                task_id,
                &[
                    plan.source.clone(),
                    plan.destination.clone(),
                    plan.workspace.clone(),
                ],
            )
            .map_err(|e| failure(e.to_string()))?;
        coordinator
            .bind_application(
                task_id,
                crate::coordinator::ApplicationBinding {
                    identity: None,
                    executable: plan.executable.clone(),
                    document_id: plan.document_id.clone(),
                    workspace: Some(plan.workspace.clone()),
                },
            )
            .map_err(|e| failure(e.to_string()))?;
        fs::create_dir(&plan.workspace).map_err(io_failure)?;
        let _directory = custodian_winsec::guarded_copy::DirectoryGuard::open(&plan.workspace)
            .map_err(io_failure)?;
        let journal = ApplicationJournal::create(&plan.workspace.join("journal"), plan.digest)
            .map_err(io_failure)?;
        journal
            .append(Event::Scope {
                scope: crate::app_journal::RecoveryScope {
                    source: plan.source.clone(),
                    destination: plan.destination.clone(),
                    source_identity: plan.source_identity,
                    source_digest: plan.source_digest,
                    expected: plan.expected.clone(),
                },
            })
            .map_err(io_failure)?;
        let mut dispatch = journal.clone();
        let cells = matches!(plan.goal, ApplicationGoal::Cells { .. });
        let source = plan
            .workspace
            .join(if cells { "source.xlsx" } else { "document.txt" });
        let mut copy = custodian_winsec::guarded_copy::CopyGuard::open(&plan.source, &source)
            .map_err(io_failure)?;
        journal
            .append(Event::StagingPending {
                source_digest: plan.source_digest,
            })
            .map_err(io_failure)?;
        copy.copy_verified_with_progress(
            plan.source_bytes,
            |input| {
                Ok(crate::hashing::hash_reader(input)?
                    == blake3::Hash::from_bytes(plan.source_digest)
                        .to_hex()
                        .as_str())
            },
            &mut |_| {
                if cancel.load(Ordering::Acquire)
                    || !crate::privacy::request_is_current()
                    || authority
                        .admit_document_source(&plan.executable, &plan.document_id)
                        .is_err()
                {
                    return Err(std::io::Error::other("Application staging stopped"));
                }
                Ok(())
            },
        )
        .map_err(io_failure)?;
        drop(copy);
        journal
            .append(Event::Staged {
                source_digest: plan.source_digest,
            })
            .map_err(io_failure)?;
        let launch = authority
            .issue_launch_after_native_consent(&plan.executable, Duration::from_secs(1800))
            .map_err(failure)?;
        journal
            .append(Event::LaunchPending {
                executable_digest: *blake3::hash(plan.executable.as_bytes()).as_bytes(),
            })
            .map_err(io_failure)?;
        let mut spawned = |pid, birth| {
            journal
                .append(Event::OwnedProcess { pid, birth })
                .map_err(io_failure)
        };
        let session = match programs {
            Programs::Editor(program) if !cells && program.executable() == plan.executable => {
                Session::Editor(EditorSession::launch(
                    program,
                    &plan.workspace,
                    &source,
                    authority,
                    &launch,
                    cancel,
                    &plan.document_id,
                    &mut dispatch,
                    &mut spawned,
                )?)
            }
            Programs::Excel(program) if cells && program.executable() == plan.executable => {
                Session::Excel(ExcelSession::launch(
                    program,
                    &plan.workspace,
                    authority,
                    &launch,
                    cancel,
                    &plan.document_id,
                    &mut dispatch,
                    &mut spawned,
                )?)
            }
            _ => {
                return Err(failure(
                    "Application program differs from the approved native target",
                ))
            }
        };
        let stage = if cells {
            plan.workspace.join("output.xlsx")
        } else {
            source
        };
        let binding = session.request(ApplicationAction::OpenBoundDocument);
        coordinator
            .bind_application(
                task_id,
                crate::coordinator::ApplicationBinding {
                    identity: Some(binding.identity),
                    executable: plan.executable.clone(),
                    document_id: binding.document_id,
                    workspace: Some(plan.workspace.clone()),
                },
            )
            .map_err(|e| failure(e.to_string()))?;
        let mut runner = Runner {
            plan,
            authority,
            session,
            journal,
            files,
            stage,
            index: 0,
            receipt: None,
            closed: false,
        };
        coordinator.run_program(task_id, program(cells), cancel, &mut runner)
    }
}
