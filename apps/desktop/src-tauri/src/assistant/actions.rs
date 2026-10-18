//! Actions the language model asked for that wait for the person's OK.
//!
//! When the person types "search cheap flights", that is their own request
//! and it runs at once. When the *model* decides to search the web, open a
//! website or undo a clean-up, Cubby shows a card instead, and nothing is sent
//! or changed until the person clicks it (Rule 4: only the person raises
//! Cubby's authority; K-31, K-33). Each card can be used once and expires.

use super::reply;
use super::tools::ToolCall;
use custodian_core::file_tasks::FileTaskPlan;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a card stays usable.
pub const CARD_LIFETIME: Duration = Duration::from_secs(10 * 60);

/// What the chat panel shows for a waiting action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionCard {
    pub id: String,
    pub title: String,
    pub detail: String,
    /// The label of the button that allows it.
    pub confirm: String,
}

/// The card for `call`, or `None` when it runs without asking.
pub fn card_text(call: &ToolCall) -> Option<(String, String, String)> {
    Some(match call {
        ToolCall::WebSearch(query) => (
            "Search the web?".into(),
            format!(
                "Search Google for \"{}\" in your browser. Only these words are sent.",
                reply::safe_name(query)
            ),
            "Search".into(),
        ),
        ToolCall::OpenWebsite { url, .. } => (
            "Open a website?".into(),
            format!("Open {} in your browser.", reply::safe_name(url)),
            "Open".into(),
        ),
        ToolCall::UndoLastCleanup => (
            "Undo the last file task?".into(),
            "Undo the most recent verified file task or clean-up. Changed files and occupied original locations are left alone.".into(),
            "Put back".into(),
        ),
        ToolCall::OpenProgram(name) => (
            "Open an app?".into(),
            format!("Open {}.", reply::safe_name(name)),
            "Open".into(),
        ),
        ToolCall::OpenFile(name) => (
            "Open a file?".into(),
            format!(
                "Open your file named like \"{}\" with its usual app.",
                reply::safe_name(name)
            ),
            "Open".into(),
        ),
        ToolCall::CloseApp(name) => (
            "Close an app?".into(),
            format!(
                "Close {}. If it has unsaved work, it will ask you first.",
                reply::safe_name(name)
            ),
            "Close".into(),
        ),
        // A setting is shown to the person even when they asked for it
        // themselves: getting it wrong changes their desktop. The value and
        // the setting are both from the catalogue, so neither can be invented.
        ToolCall::ChangeSetting { setting, value } => {
            let known = custodian_winsec::settings::find(setting)?;
            let wanted = known.choice(value)?;
            let from = match known.current() {
                Some(now) => format!("now {}", now.key),
                None => "Windows has no value for it yet".to_string(),
            };
            (
                "Change a Windows setting?".into(),
                format!(
                    "{}: set it to {} ({from})? {} {}\nNothing changes until you allow \
                     it, and I'll show you how to put it back.",
                    known.label, wanted.key, known.effect, known.applies
                ),
                "Change it".into(),
            )
        }
        ToolCall::MoveFile {
            source,
            destination,
        } => (
            "Move file?".into(),
            format!(
                "Move \"{}\" to \"{}\". Nothing changes until you allow it.",
                reply::safe_name(source),
                reply::safe_name(destination)
            ),
            "Move".into(),
        ),
        ToolCall::CopyFile {
            source,
            destination,
        } => (
            "Copy file?".into(),
            format!(
                "Copy \"{}\" to \"{}\". Nothing changes until you allow it.",
                reply::safe_name(source),
                reply::safe_name(destination)
            ),
            "Copy".into(),
        ),
        ToolCall::RenameFile { path, new_name } => (
            "Rename file?".into(),
            format!(
                "Rename \"{}\" to \"{}\". Nothing changes until you allow it.",
                reply::safe_name(path),
                reply::safe_name(new_name)
            ),
            "Rename".into(),
        ),
        ToolCall::CreateFolder(path) => (
            "Create folder?".into(),
            format!(
                "Create folder \"{}\". Nothing changes until you allow it.",
                reply::safe_name(path)
            ),
            "Create".into(),
        ),
        _ => return None,
    })
}

/// Human-readable description of what `call` is asking to do.
pub fn action_name(call: &ToolCall) -> String {
    match call {
        ToolCall::WebSearch(query) => format!("searching the web for \"{query}\""),
        ToolCall::OpenWebsite { url, .. } => format!("opening {url}"),
        ToolCall::UndoLastCleanup => "undoing the last verified file task or clean-up".into(),
        ToolCall::CloseApp(name) => format!("closing {name}"),
        ToolCall::ChangeSetting { setting, value } => {
            format!("changing the Windows setting {setting} to {value}")
        }
        ToolCall::OpenProgram(name) => format!("opening {name}"),
        ToolCall::OpenFile(name) => format!("opening the file {name}"),
        ToolCall::MoveFile {
            source,
            destination,
        } => format!("moving \"{source}\" to \"{destination}\""),
        ToolCall::CopyFile {
            source,
            destination,
        } => format!("copying \"{source}\" to \"{destination}\""),
        ToolCall::RenameFile { path, new_name } => format!("renaming \"{path}\" to \"{new_name}\""),
        ToolCall::CreateFolder(path) => format!("creating folder \"{path}\""),
        ToolCall::GetFileInfo(path) => format!("getting details for \"{path}\""),
        _ => "this action".into(),
    }
}

/// What the model is told while the card waits.
pub fn waiting_note(call: &ToolCall) -> String {
    let what = action_name(call);
    format!("Waiting for the person: a card asks them to allow {what}. Nothing has happened yet.")
}

/// What Cubby says when the person declines a card.
pub fn declined_text(call: &ToolCall) -> String {
    match call {
        ToolCall::WebSearch(_) => "Okay, I won't search for that.".into(),
        ToolCall::OpenWebsite { .. } => "Okay, I won't open it.".into(),
        ToolCall::UndoLastCleanup => "Okay, I'll leave everything as it is.".into(),
        ToolCall::CloseApp(_) => "Okay, I'll leave it open.".into(),
        ToolCall::ChangeSetting { .. } => "Okay, I'll leave that setting as it is.".into(),
        ToolCall::OpenProgram(_) | ToolCall::OpenFile(_) => "Okay, I won't open it.".into(),
        ToolCall::MoveFile { .. } => "Okay, I won't move that file.".into(),
        ToolCall::CopyFile { .. } => "Okay, I won't copy that file.".into(),
        ToolCall::RenameFile { .. } => "Okay, I won't rename that file.".into(),
        ToolCall::CreateFolder(_) => "Okay, I won't create that folder.".into(),
        _ => "Okay, I won't.".into(),
    }
}

/// Waiting actions by card id.
#[derive(Default)]
pub struct PendingActions {
    waiting: Mutex<HashMap<String, PendingAction>>,
}

struct PendingAction {
    call: ToolCall,
    created: Instant,
    file_plan: Option<FileTaskPlan>,
    privacy_revision: u64,
}

impl PendingActions {
    pub fn clear(&self) {
        self.waiting
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
    /// Keeps `call` until the person decides and returns its card, or `None`
    /// if `call` needs no card.
    #[cfg(test)]
    pub fn add(&self, call: ToolCall, now: Instant) -> Option<ActionCard> {
        self.add_prepared(call, now, None)
    }

    pub fn add_prepared(
        &self,
        call: ToolCall,
        now: Instant,
        plan: Option<FileTaskPlan>,
    ) -> Option<ActionCard> {
        let (title, detail, confirm) = card_text(&call)?;
        let detail = if let Some(plan) = &plan {
            let source = plan
                .source
                .as_ref()
                .map(|path| {
                    format!(
                        "Source: {}\n",
                        reply::safe_name(&path.display().to_string())
                    )
                })
                .unwrap_or_default();
            format!("{source}Destination: {}\nNothing changes until you allow it. If the file or destination folder changes, this request will be refused. A verified undo record is saved before the change.", reply::safe_name(&plan.destination.display().to_string()))
        } else {
            detail
        };
        let id = uuid_like();
        let mut waiting = self.waiting.lock().unwrap_or_else(|e| e.into_inner());
        waiting.retain(|_, action| now.saturating_duration_since(action.created) < CARD_LIFETIME);
        waiting.insert(
            id.clone(),
            PendingAction {
                call,
                created: now,
                file_plan: plan,
                privacy_revision: custodian_core::privacy::request_revision(),
            },
        );
        Some(ActionCard {
            id,
            title,
            detail,
            confirm,
        })
    }

    /// Removes and returns the action for `id` if it is still valid. A card
    /// can be used once.
    #[cfg(test)]
    pub fn take(&self, id: &str, now: Instant) -> Option<ToolCall> {
        self.take_prepared(id, now).map(|(call, _)| call)
    }

    pub fn take_prepared(
        &self,
        id: &str,
        now: Instant,
    ) -> Option<(ToolCall, Option<FileTaskPlan>)> {
        let action = self
            .waiting
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id)?;
        (now.saturating_duration_since(action.created) < CARD_LIFETIME
            && action.privacy_revision == custodian_core::privacy::current().revision()
            && custodian_core::privacy::request_is_current())
        .then_some((action.call, action.file_plan))
    }
}

/// A random 96-bit id as hex.
fn uuid_like() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_published_cards_keep_original_privacy_revision() {
        let actions = PendingActions::default();
        let policy = std::sync::Arc::new(custodian_core::privacy::AccessPolicy::default());
        let queued = policy.revision();
        policy.replace_private_paths(vec![]).unwrap();
        let now = Instant::now();
        let card = custodian_core::privacy::with_policy_revision(policy.clone(), queued, || {
            actions.add(ToolCall::UndoLastCleanup, now).unwrap()
        });
        custodian_core::privacy::with_policy(policy, || {
            assert!(actions.take(&card.id, now).is_none());
        });
    }

    #[test]
    fn cards_only_for_actions_that_send_or_change_something() {
        assert!(card_text(&ToolCall::WebSearch("weather".into())).is_some());
        assert!(card_text(&ToolCall::UndoLastCleanup).is_some());
        assert!(card_text(&ToolCall::OpenWebsite {
            name: "youtube".into(),
            url: "https://www.youtube.com".into()
        })
        .is_some());
        assert!(card_text(&ToolCall::CloseApp("Chrome".into())).is_some());
        assert!(card_text(&ToolCall::ChangeSetting {
            setting: "dark_mode_apps".into(),
            value: "dark".into()
        })
        .is_some());
        // A setting or a value that is not in the catalogue makes no card at all.
        assert!(card_text(&ToolCall::ChangeSetting {
            setting: "no_such_setting".into(),
            value: "on".into()
        })
        .is_none());
        assert!(card_text(&ToolCall::ChangeSetting {
            setting: "dark_mode_apps".into(),
            value: "sideways".into()
        })
        .is_none());
        assert!(card_text(&ToolCall::ListOpenApps).is_none());
        assert!(card_text(&ToolCall::CheckDiskSpace).is_none());
        assert!(card_text(&ToolCall::SystemStatus).is_none());
    }

    #[test]
    fn a_card_works_once_and_expires() {
        let actions = PendingActions::default();
        let t0 = Instant::now();
        let card = actions
            .add(ToolCall::WebSearch("cheap flights".into()), t0)
            .unwrap();
        assert_eq!(card.confirm, "Search");
        assert!(card.detail.contains("\"cheap flights\""));
        assert_eq!(
            actions.take(&card.id, t0),
            Some(ToolCall::WebSearch("cheap flights".into()))
        );
        assert_eq!(actions.take(&card.id, t0), None, "used once");

        let card = actions.add(ToolCall::UndoLastCleanup, t0).unwrap();
        assert_eq!(actions.take(&card.id, t0 + CARD_LIFETIME), None, "expired");
        assert_eq!(actions.take("made-up", t0), None);
        assert!(actions.add(ToolCall::CheckDiskSpace, t0).is_none());
    }

    #[test]
    fn card_text_cannot_be_disguised() {
        let (_, detail, _) = card_text(&ToolCall::WebSearch("pay\u{202e}lanif".into())).unwrap();
        assert!(!detail.contains('\u{202e}'));
    }

    #[test]
    fn tells_the_model_nothing_happened_yet() {
        let note = waiting_note(&ToolCall::WebSearch("weather in Delhi".into()));
        assert!(note.contains("weather in Delhi"));
        assert!(note.contains("Nothing has happened yet"));
    }
}
