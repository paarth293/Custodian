//! Model-owned interpretation, native-owned binding and capability compilation.
use crate::workflow::{Filter, Proposal, Selection};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub op: String,
    pub after: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentPlan {
    pub decision: String,
    pub filter: Filter,
    pub largest: bool,
    pub source: usize,
    pub destination: usize,
    pub nodes: Vec<Node>,
    pub completion: String,
    #[serde(default)]
    pub attempts: u8,
}
pub fn unsupported(message: &str) -> bool {
    let clean = message
        .replace("LOCATION_0", "")
        .replace("LOCATION_1", "")
        .to_ascii_lowercase();
    let words = clean
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let blocked = [
        "not",
        "don",
        "never",
        "no",
        "until",
        "stop",
        "cancel",
        "abort",
        "pause",
        "open",
        "launch",
        "close",
        "edit",
        "write",
        "read",
        "summarize",
        "archive",
        "compress",
        "extract",
        "convert",
        "install",
        "uninstall",
        "encrypt",
        "decrypt",
        "reboot",
        "restart",
        "settings",
        "organize",
        "sort",
        "schedule",
        "watch",
        "monitor",
        "those",
        "except",
        "excluding",
        "exclude",
        "invoices",
        "containing",
        "contains",
        "password",
        "passwords",
        "newer",
        "older",
        "yesterday",
        "today",
        "tomorrow",
        "recent",
        "recently",
        "modified",
        "since",
        "before",
        "after",
        "unless",
        "if",
        "only",
        "smallest",
        "newest",
        "delete",
        "erase",
        "remove",
        "move",
        "rename",
        "wipe",
        "overwrite",
        "replace",
        "email",
        "upload",
        "download",
        "sendmail",
        "execute",
        "run",
        "powershell",
        "terminal",
        "script",
        "ignore",
    ];
    !words.iter().any(|w| {
        [
            "copy",
            "copies",
            "copying",
            "backup",
            "back",
            "duplicate",
            "collect",
            "gather",
            "preserve",
        ]
        .contains(w)
    }) || !message.is_ascii()
        || words
            .iter()
            .any(|w| blocked.contains(w) || w.chars().all(|c| c.is_ascii_digit()))
        || (words
            .iter()
            .any(|w| ["documents", "document", "docs", "paperwork"].contains(w))
            && words
                .iter()
                .any(|w| ["images", "image", "photos", "photo", "pictures", "picture"].contains(w)))
}
impl IntentPlan {
    pub fn compile(&self, message: &str) -> Result<(Selection, Proposal), String> {
        if unsupported(message) {
            return Err("This request has an unsupported constraint or effect. Clarify the goal; no substitute collection was executed.".into());
        }
        if self.decision != "copy" {
            return Err("Please specify an unambiguous supported file-copy goal and its source and destination. No work was executed.".into());
        }
        if message.len() > 2400
            || self.source > 1
            || self.destination > 1
            || self.source == self.destination
            || !(1..=2).contains(&self.attempts)
            || self.completion != "all_copies_verified"
            || self.nodes.len() > 5
            || self
                .nodes
                .iter()
                .enumerate()
                .any(|(i, n)| n.after != i as i32 - 1)
        {
            return Err("Invalid grounded plan, dependencies or completion predicate".into());
        }
        let selection = Selection {
            filter: self.filter,
            largest: self.largest,
        };
        let lower = message
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();
        for i in 0..2 {
            if lower.contains(&format!("from location_{i}")) && self.source != i {
                return Err("The plan reversed the explicit source location".into());
            }
            if ["to", "into"]
                .iter()
                .any(|p| lower.contains(&format!("{p} location_{i}")))
                && self.destination != i
            {
                return Err("The plan reversed the explicit destination location".into());
            }
        }
        let proposal = Proposal {
            steps: self.nodes.iter().map(|n| n.op.clone()).collect(),
            attempts: self.attempts,
        };
        proposal.validate(selection).map_err(|e| e.to_string())?;
        // An explicit category is a native consistency check, never inferred from paths.
        let words = message.to_ascii_lowercase();
        let words = words
            .split(|c: char| !c.is_ascii_alphabetic())
            .collect::<Vec<_>>();
        let docs = words
            .iter()
            .any(|w| ["documents", "document", "docs", "paperwork"].contains(w));
        let images = words
            .iter()
            .any(|w| ["images", "image", "photos", "photo", "pictures", "picture"].contains(w));
        if (docs && self.filter != Filter::Documents)
            || (images && self.filter != Filter::Images)
            || (words.iter().any(|w| ["largest", "biggest"].contains(w)) != self.largest)
        {
            return Err("The interpretation omitted an explicit selection constraint".into());
        }
        Ok((selection, proposal))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unknown_effects_dependencies_and_omitted_constraints() {
        let mut plan = IntentPlan {
            decision: "copy".into(),
            filter: Filter::Documents,
            largest: false,
            source: 0,
            destination: 1,
            nodes: [
                "discover",
                "filter_documents",
                "select_all",
                "copy",
                "verify",
            ]
            .iter()
            .enumerate()
            .map(|(i, op)| Node {
                op: (*op).into(),
                after: i as i32 - 1,
            })
            .collect(),
            completion: "all_copies_verified".into(),
            attempts: 1,
        };
        assert!(plan
            .compile("Collect documents from LOCATION_0 to LOCATION_1")
            .is_ok());
        assert!(plan.compile("Collect documents except invoices").is_err());
        assert!(plan.compile("Collect the biggest documents").is_err());
        plan.nodes[3].op = "delete".into();
        assert!(plan.compile("Collect documents").is_err());
        plan.nodes[3].op = "copy".into();
        plan.nodes[3].after = 4;
        assert!(plan.compile("Collect documents").is_err());
    }
    #[test]
    fn frozen_negative_guards_are_not_silent_constraint_dropping() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/EVIDENCE/stage-3/intent-cases.json"
        ))
        .unwrap();
        for case in cases["cases"].as_array().unwrap() {
            if case["expected"]["must_not_execute"] == true {
                assert!(
                    unsupported(case["message"].as_str().unwrap()),
                    "{}",
                    case["id"]
                );
            }
        }
    }
    #[test]
    fn compound_unregistered_effects_cannot_be_discarded_from_a_copy_goal() {
        for goal in [
            "Do not copy documents yet",
            "Stop copying documents",
            "Open Notepad and copy documents",
            "Copy documents and edit the originals",
            "Copy files and change settings",
            "Copy photos then upload them",
        ] {
            assert!(unsupported(goal));
        }
    }
    #[test]
    #[ignore = "requires completed actual local inference report"]
    fn actual_frozen_model_results_pass_the_independent_native_compiler() {
        let path = std::env::var("CUSTODIAN_INTENT_REPORT").expect("set completed report path");
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let frozen: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/EVIDENCE/stage-3/intent-cases.json"
        ))
        .unwrap();
        let rows = report["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 36);
        let mut seen = std::collections::BTreeSet::new();
        let mut positive = 0;
        for row in rows {
            assert!(seen.insert(row["id"].as_str().unwrap()));
            let case = frozen["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|case| case["id"] == row["id"])
                .unwrap();
            let parsed = serde_json::from_value::<IntentPlan>(row["proposal"].clone());
            let compiled = parsed
                .as_ref()
                .ok()
                .and_then(|plan| plan.compile(case["message"].as_str().unwrap()).ok());
            if case["expected"]["must_not_execute"] == true {
                assert!(compiled.is_none(), "{}", case["id"]);
                continue;
            }
            if let (Ok(plan), Some((selection, proposal))) = (parsed, compiled) {
                assert_eq!(
                    plan.source as u64,
                    case["expected"]["source"].as_u64().unwrap()
                );
                assert_eq!(
                    plan.destination as u64,
                    case["expected"]["destination"].as_u64().unwrap()
                );
                let actual_filter = serde_json::to_value(plan.filter).unwrap();
                if actual_filter == case["expected"]["filter"]
                    && plan.largest == case["expected"]["largest"].as_bool().unwrap()
                {
                    positive += 1;
                    use crate::{
                        coordinator::{TaskCoordinator, TaskState},
                        file_tasks::FileTasks,
                        routine::RoutinePolicy,
                        workflow::{prepare, Goal, WorkflowStore},
                    };
                    let root = tempfile::tempdir().unwrap();
                    let locations = [
                        root.path().join("location-0"),
                        root.path().join("location-1"),
                    ];
                    for location in &locations {
                        std::fs::create_dir(location).unwrap();
                    }
                    let source = &locations[plan.source];
                    let destination = &locations[plan.destination];
                    for (name, bytes) in [
                        ("a.txt", 100),
                        ("b.pdf", 200),
                        ("c.jpg", 300),
                        ("d.png", 50),
                    ] {
                        std::fs::write(source.join(name), vec![b'x'; bytes]).unwrap();
                    }
                    let policy = RoutinePolicy::default();
                    policy.update_permissions(0, false, Some(true)).unwrap();
                    let cancel = std::sync::atomic::AtomicBool::new(false);
                    let goal = Goal {
                        roots: vec![source.clone()],
                        destination: destination.clone(),
                        selection,
                    };
                    let prepared = prepare(
                        &policy,
                        policy.revision(),
                        &goal,
                        proposal,
                        &cancel,
                        &|_| true,
                        &mut |_| {},
                    )
                    .unwrap();
                    let coordinator = TaskCoordinator::new();
                    let (task, cancel) = coordinator
                        .create_task(
                            "Model-selected real file fixture".into(),
                            TaskState::Running,
                            &locations,
                        )
                        .unwrap();
                    let files = FileTasks::new(root.path().join("receipts"));
                    let store = WorkflowStore::new(root.path().join("workflows"));
                    let record = store
                        .execute_coordinated(
                            (&coordinator, &task),
                            &policy,
                            prepared,
                            &files,
                            (&cancel, &mut |_| {}),
                        )
                        .unwrap();
                    let count = if selection.largest {
                        1
                    } else if selection.filter == Filter::Any {
                        4
                    } else {
                        2
                    };
                    assert_eq!(record.steps.len(), count);
                    for step in &record.steps {
                        assert_eq!(
                            std::fs::read(&step.source).unwrap(),
                            std::fs::read(&step.destination).unwrap()
                        );
                    }
                    store.undo_latest(&files, &cancel).unwrap();
                    assert_eq!(std::fs::read_dir(destination).unwrap().count(), 0);
                    assert_eq!(std::fs::read_dir(source).unwrap().count(), 4);
                    println!(
                        "MODEL FILE EXECUTED: {} ({count} verified copies and undo)",
                        case["id"]
                    );
                }
            }
        }
        assert!(
            positive >= 22,
            "{positive} supported requests compiled correctly"
        );
    }
}
