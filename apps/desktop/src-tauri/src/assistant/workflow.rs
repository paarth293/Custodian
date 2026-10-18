//! Model proposes capabilities; the native goal binds authority, selection and paths.
use super::*;
use custodian_core::{
    coordinator::TaskState,
    workflow::{self as engine, Filter, Goal, Selection, WorkflowStore},
};

#[derive(Debug, PartialEq, Eq)]
struct Request {
    source: String,
    destination: PathBuf,
    selection: Selection,
    description: String,
}

fn parse(message: &str) -> Option<Result<Request, &'static str>> {
    let parts: Vec<_> = message.trim().split('"').collect();
    let outside = parts
        .iter()
        .step_by(2)
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let words: Vec<_> = outside
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect();
    let has = |choices: &[&str]| words.iter().any(|w| choices.contains(w));
    if !has(&["collect", "gather", "copy", "backup"]) && !(has(&["back"]) && has(&["up"])) {
        return None;
    }
    let documents = has(&["document", "documents", "docs"]);
    let images = has(&["image", "images", "photo", "photos", "picture", "pictures"]);
    let largest = has(&["largest", "biggest"]);
    // Keep an ordinary singular file-copy request on its existing approved
    // route; only explicit collections or largest-file goals enter this engine.
    if !largest
        && has(&["copy"])
        && !has(&[
            "collect",
            "gather",
            "documents",
            "docs",
            "images",
            "photos",
            "pictures",
            "all",
        ])
    {
        return None;
    }
    if !documents && !images && !largest && !has(&["collect", "gather"]) {
        return None;
    }
    let usage = "Specify one file collection and two quoted existing absolute folders, for example: Collect documents from \"C:\\Projects\" to \"D:\\Backups\". Photos and largest-file selection are also supported. Archives, content-based filtering and moving originals are not supported by this workflow.";
    let allowed = [
        "please",
        "can",
        "could",
        "you",
        "copy",
        "back",
        "up",
        "backup",
        "collect",
        "gather",
        "my",
        "the",
        "all",
        "largest",
        "biggest",
        "document",
        "documents",
        "docs",
        "image",
        "images",
        "picture",
        "pictures",
        "photo",
        "photos",
        "files",
        "file",
        "from",
        "in",
        "to",
        "into",
        "a",
        "for",
        "me",
        "of",
    ];
    if outside.len() > 1600
        || (largest && has(&["all"]))
        || (largest && has(&["documents", "docs", "images", "photos", "pictures", "files"]))
        || (!largest && !has(&["documents", "docs", "images", "photos", "pictures", "all"]))
        || words.iter().any(|w| !allowed.contains(w))
        || documents && images
        || !documents && !images && !largest
        || parts.len() != 5
        || !parts[4].trim().is_empty()
        || !matches!(parts[2].trim().to_ascii_lowercase().as_str(), "to" | "into")
        || !parts[0]
            .split_whitespace()
            .last()
            .is_some_and(|w| matches!(w.to_ascii_lowercase().as_str(), "from" | "in"))
        || !Path::new(parts[3]).is_absolute()
        || !(Path::new(parts[1]).is_absolute() || parts[1].eq_ignore_ascii_case("all drives"))
    {
        return Some(Err(usage));
    }
    Some(Ok(Request {
        source: parts[1].into(),
        destination: parts[3].into(),
        selection: Selection {
            filter: if documents {
                Filter::Documents
            } else if images {
                Filter::Images
            } else {
                Filter::Any
            },
            largest,
        },
        description: format!("{} SOURCE {} DESTINATION", parts[0].trim(), parts[2].trim()),
    }))
}

fn store(app: &AppHandle) -> Result<WorkflowStore, String> {
    Ok(WorkflowStore::new(
        crate::paths::data_dir(app)
            .map_err(|e| e.to_string())?
            .join("workflows"),
    ))
}

fn propose(
    app: &AppHandle,
    state: &AssistantState,
    goal: &str,
    selection: Selection,
    cancel: &AtomicBool,
) -> Result<engine::Proposal, String> {
    let mut agent = state.agent.lock().unwrap_or_else(|e| e.into_inner());
    let revision = state.privacy.revision();
    if state.agent_policy_revision.load(Ordering::Acquire) != revision {
        *agent = None;
        state
            .agent_policy_revision
            .store(revision, Ordering::Release);
    }
    if cancel.load(Ordering::Relaxed) || !custodian_core::privacy::request_is_current() {
        return Err("Workflow planning stopped".into());
    }
    ensure_agent(app, &mut agent, Some(cancel))?;
    let result = agent
        .as_mut()
        .ok_or("Local planner unavailable")?
        .workflow_plan(goal, selection, cancel)
        .map_err(|e| e.to_string());
    if result.is_err() {
        *agent = None;
    }
    mark_agent_used(state);
    result
}

fn status(record: &engine::Record) -> String {
    let paths = record
        .steps
        .iter()
        .map(|step| {
            format!(
                "{} {}",
                if step.undone {
                    "Undone:"
                } else if step.verified {
                    "Verified:"
                } else {
                    "Unverified:"
                },
                step.destination.display()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("Workflow {}: {:?}.\n{}\n{}\nSay \"undo workflow\" to remove only its verified copies. Partial or changed outputs are retained for review.",record.id,record.state,record.detail,paths)
}

pub(super) fn handle(
    app: &AppHandle,
    state: &AssistantState,
    message: &str,
    channel: Sink<'_>,
    revision: u64,
) -> bool {
    let command = message.trim().trim_end_matches('.').to_ascii_lowercase();
    if matches!(
        command.as_str(),
        "show workflow status" | "workflow status" | "undo workflow"
    ) {
        let result = (|| -> Result<String, String> {
            let files = file_ops::store(app)?;
            let store = store(app)?;
            if command == "undo workflow" {
                let latest = store
                    .records()
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .next()
                    .ok_or("No saved workflow")?;
                for step in &latest.steps {
                    custodian_core::privacy::check(&step.destination).map_err(|e| e.to_string())?;
                }
                let paths = latest
                    .steps
                    .iter()
                    .map(|s| s.destination.clone())
                    .collect::<Vec<_>>();
                let (task, cancel) = state
                    .create_task(
                        "Undoing a verified workflow".into(),
                        TaskState::Running,
                        &paths,
                    )
                    .map_err(|e| e.to_string())?;
                let result = store.undo_latest(&files, &cancel);
                state
                    .coordinator
                    .update_state(
                        &task,
                        if result.is_ok() {
                            TaskState::Completed
                        } else {
                            TaskState::RecoveryRequired
                        },
                        None,
                    )
                    .ok();
                result
                    .map(|r| format!("Workflow undo verified.\n{}", status(&r)))
                    .map_err(|e| e.to_string())
            } else {
                store
                    .inspect(&files)
                    .map(|r| {
                        r.map(|r| status(&r))
                            .unwrap_or_else(|| "No saved workflow".into())
                    })
                    .map_err(|e| e.to_string())
            }
        })();
        say(channel,result.unwrap_or_else(|e|format!("Workflow inspection/undo could not complete: {e}. No unverified output was removed.")));
        return true;
    }
    let mut admission = None;
    let mut interpreted_proposal = None;
    let parsed = if let Some(grounded) = language::Grounded::probe(message) {
        let grounded = match grounded {
            Ok(g) => g,
            Err(e) => {
                say(channel, e);
                return true;
            }
        };
        if state.routine.with_collection(revision, || Ok(())).is_err() {
            say(channel,"Automatic collections are off or their permission changed. Enable verified collections in Routine permissions, then ask again.");
            return true;
        }
        let resources = grounded
            .paths
            .iter()
            .filter(|p| !p.eq_ignore_ascii_case("all drives"))
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        let (task, cancel) = match state.create_task(
            "Interpreting and executing a grounded file goal".into(),
            TaskState::Running,
            &resources,
        ) {
            Ok(t) => t,
            Err(e) => {
                say(channel, e.to_string());
                return true;
            }
        };
        let result = (|| -> Result<Request, String> {
            let mut agent = state.agent.lock().unwrap_or_else(|e| e.into_inner());
            if cancel.load(Ordering::Relaxed) || !custodian_core::privacy::request_is_current() {
                return Err("Planning stopped".into());
            }
            let privacy_revision = state.privacy.revision();
            if state.agent_policy_revision.load(Ordering::Acquire) != privacy_revision {
                *agent = None;
                state
                    .agent_policy_revision
                    .store(privacy_revision, Ordering::Release);
            }
            ensure_agent(app, &mut agent, Some(&cancel))?;
            emit(
                channel,
                ChatEvent::Status {
                    text: "Interpreting the goal and composing registered file capabilities…"
                        .into(),
                },
            );
            let result = agent
                .as_mut()
                .ok_or("Local interpreter unavailable")?
                .intent_plan(&grounded.message, &cancel);
            if result.is_err() {
                *agent = None;
            }
            mark_agent_used(state);
            let plan = result.map_err(|e| e.to_string())?;
            grounded.validate()?;
            if cancel.load(Ordering::Relaxed) {
                return Err("Planning stopped".into());
            }
            let (selection, proposal) = plan.compile(&grounded.message)?;
            if grounded.paths[plan.destination].eq_ignore_ascii_case("all drives") {
                return Err("A destination must be one measured directory".into());
            }
            let request = Request {
                source: grounded.paths[plan.source].clone(),
                destination: grounded.paths[plan.destination].clone().into(),
                selection,
                description: grounded.message.clone(),
            };
            interpreted_proposal = Some(proposal);
            Ok(request)
        })();
        match result {
            Ok(request) => {
                admission = Some((task, cancel));
                Some(Ok(request))
            }
            Err(error) => {
                state
                    .coordinator
                    .update_state(
                        &task,
                        if cancel.load(Ordering::Relaxed) {
                            TaskState::Cancelled
                        } else {
                            TaskState::Failed
                        },
                        Some(error.clone()),
                    )
                    .ok();
                say(
                    channel,
                    format!(
                        "Workflow not completed: {error}. Nothing was copied for this request."
                    ),
                );
                return true;
            }
        }
    } else {
        parse(message)
    };
    let Some(parsed) = parsed else {
        return false;
    };
    let request = match parsed {
        Ok(request) => request,
        Err(error) => {
            say(channel, error);
            return true;
        }
    };
    if state.routine.with_collection(revision, || Ok(())).is_err() {
        if let Some((id, _)) = &admission {
            state
                .coordinator
                .update_state(id, TaskState::Cancelled, None)
                .ok();
        }
        say(channel,"Automatic collections are off or their permission changed. Enable verified collections in Routine permissions, then ask again. The single-file backup permission does not enable collections.");
        return true;
    }
    if custodian_core::privacy::check(&request.destination).is_err() {
        if let Some((id, _)) = &admission {
            state
                .coordinator
                .update_state(id, TaskState::Cancelled, None)
                .ok();
        }
        say(channel, custodian_core::privacy::DENIED);
        return true;
    }
    let scope = if request.source.eq_ignore_ascii_case("all drives") {
        Scope::AllDrives
    } else {
        Scope::Custom(request.source.clone().into())
    };
    let goal = Goal {
        roots: resolve_scope(app, &scope)
            .into_iter()
            .map(|(_, p)| p)
            .collect(),
        destination: request.destination,
        selection: request.selection,
    };
    let (task, cancel) = match admission.map(Ok).unwrap_or_else(|| {
        state.create_task(
            "Planning and executing a verified file collection".into(),
            TaskState::Running,
            std::slice::from_ref(&goal.destination),
        )
    }) {
        Ok(task) => task,
        Err(e) => {
            say(channel, e.to_string());
            return true;
        }
    };
    let mut executing = false;
    let run = (|| -> Result<String, String> {
        emit(
            channel,
            ChatEvent::Status {
                text: "Qwen is planning a bounded file workflow…".into(),
            },
        );
        let proposal = match interpreted_proposal.take() {
            Some(proposal) => proposal,
            None => propose(app, state, &request.description, goal.selection, &cancel)?,
        };
        proposal
            .validate(goal.selection)
            .map_err(|e| e.to_string())?;
        say(channel,format!("Plan validated after {} model attempt(s): {}. Paths and permission are bound by the desktop; nothing copied yet.",proposal.attempts,proposal.steps.join(" → ")));
        emit(
            channel,
            ChatEvent::Status {
                text: "Discovering permitted files for the workflow…".into(),
            },
        );
        let prepared = engine::prepare(
            &state.routine,
            revision,
            &goal,
            proposal,
            &cancel,
            &routine::eligible,
            &mut |count| {
                emit(
                    channel,
                    ChatEvent::Status {
                        text: format!("Workflow discovery: checked {count} files…"),
                    },
                )
            },
        )
        .map_err(|e| e.to_string())?;
        let (scanned, excluded, inaccessible) =
            (prepared.scanned, prepared.excluded, prepared.inaccessible);
        let files = file_ops::store(app)?;
        let workflow_store = store(app)?;
        executing = true;
        let record = workflow_store
            .execute_coordinated(
                (&state.coordinator, &task),
                &state.routine,
                prepared,
                &files,
                (&cancel, &mut |r| {
                    emit(
                        channel,
                        ChatEvent::Status {
                            text: r.detail.clone(),
                        },
                    )
                }),
            )
            .map_err(|e| e.to_string())?;
        let note = record_history(
            app,
            json!({"kind":"workflow","id":record.id,"files":record.steps.len(),"at":now_unix()}),
        );
        Ok(with_note(format!("Workflow verified: {} copies; originals preserved. Checked {scanned} files, with {excluded} exclusions and {inaccessible} inaccessible locations. Saved step receipts support inspection and group undo after restart.\n{}",record.steps.len(),status(&record)),note))
    })();
    match run {
        Ok(text) => {
            state
                .coordinator
                .update_state(&task, TaskState::Completed, None)
                .ok();
            say(channel, text);
        }
        Err(error) => {
            state
                .coordinator
                .update_state(
                    &task,
                    if executing {
                        TaskState::RecoveryRequired
                    } else if cancel.load(Ordering::Relaxed) {
                        TaskState::Cancelled
                    } else {
                        TaskState::Failed
                    },
                    None,
                )
                .ok();
            say(
                channel,
                if executing {
                    format!("Workflow not completed: {error}. No completion is claimed. Any completed copies remain recorded; ask \"show workflow status\" before continuing.")
                } else {
                    format!("Workflow not completed: {error}. Nothing was copied for this request; earlier saved workflows are unchanged.")
                },
            );
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_intent_binds_paths_and_rejects_unsupported_semantics() {
        for text in [
            r#"Collect documents from "C:\Projects" to "D:\Backups""#,
            r#"Please gather all my photos in "all drives" into "D:\Photos""#,
            r#"Could you copy the largest document from "C:\A" to "D:\B""#,
        ] {
            assert!(parse(text).unwrap().is_ok(), "{text}");
        }
        for text in [
            r#"Collect documents except invoices from "C:\A" to "D:\B""#,
            r#"Copy documents and photos from "C:\A" to "D:\B""#,
            r#"Collect documents from "relative" to "D:\B""#,
            r#"Collect documents from "C:\A" to "D:\B" and delete originals"#,
            r#"Copy the largest documents from "C:\A" to "D:\B""#,
        ] {
            assert!(parse(text).unwrap().is_err(), "{text}");
        }
        assert!(parse("How much free space?").is_none());
        assert!(parse(r#"Copy a document from "C:\A\report.txt" to "D:\B""#).is_none());
        let goal = parse(r#"Collect documents from "C:\images\largest" to "D:\B""#)
            .unwrap()
            .unwrap();
        assert_eq!(
            goal.selection,
            Selection {
                filter: Filter::Documents,
                largest: false
            }
        );
        assert!(!goal.description.contains("C:"));
    }
}
