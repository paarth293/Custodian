//! Qualified app goals and one-use native approval cards. Neither the webview
//! nor model supplies an executable, grant, reflected command or stored plan.
use super::*;
use custodian_core::{
    app_adapter::{CellEdit, CellInput},
    app_workflow::{execute_after_native_consent, ApplicationGoal, ApplicationPlan, Programs},
    coordinator::TaskState,
    editor_session::EditorProgram,
    excel_session::ExcelProgram,
    file_tasks::FileTasks,
};
use custodian_winsec::registry::{self, Hive};

enum Program {
    Editor(EditorProgram),
    Excel(ExcelProgram),
}
struct Prepared {
    plan: ApplicationPlan,
    program: Program,
    created: Instant,
    file_revision: u64,
    epoch: u64,
}
#[derive(Default)]
pub(super) struct Pending {
    entries: Mutex<HashMap<String, Prepared>>,
}
impl Pending {
    pub fn clear(&self) {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
    fn take(&self, id: &str) -> Option<Prepared> {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id)
    }
}
struct Request {
    source: PathBuf,
    destination: PathBuf,
    goal: ApplicationGoal,
}
const TEXT_HELP: &str = "Use: Replace \"unique text\" with \"replacement\" in \"C:\\path\\source.txt\" and save as \"D:\\path\\result.txt\". UTF-8 text up to 64 KiB is supported. The original is preserved and the destination must be new.";
const CELL_HELP: &str = "Use: Set cells in \"C:\\path\\source.xlsx\" to \"A1=12; A2=8; A3=SUM(A1:A2)\" and save as \"D:\\path\\result.xlsx\". One sheet, A1:P16, up to 32 edits; use text: for literal text. The original is preserved and the destination must be new.";
fn coordinate(s: &str) -> Option<(u16, u16)> {
    let col = *s.as_bytes().first()?;
    if !(b'A'..=b'P').contains(&col) {
        return None;
    }
    let row = s[1..].parse::<u16>().ok()?;
    if !(1..=16).contains(&row) || s[1..] != row.to_string() {
        return None;
    }
    Some((row, (col - b'A' + 1) as u16))
}
fn parse(message: &str) -> Option<Result<Request, &'static str>> {
    let message = message.trim();
    let parts = message.split('"').collect::<Vec<_>>();
    let prefix = parts[0].trim().to_ascii_lowercase();
    let text = matches!(prefix.as_str(), "replace" | "please replace");
    let cells = matches!(prefix.as_str(), "set cells in" | "please set cells in");
    if !text && !cells {
        return None;
    }
    if message.len() > 65536 {
        return Some(Err(if text { TEXT_HELP } else { CELL_HELP }));
    }
    if text {
        if parts.len() != 9
            || !parts[2].trim().eq_ignore_ascii_case("with")
            || !parts[4].trim().eq_ignore_ascii_case("in")
            || !parts[6].trim().eq_ignore_ascii_case("and save as")
            || !parts[8].trim().is_empty()
            || !Path::new(parts[5]).is_absolute()
            || !Path::new(parts[7]).is_absolute()
        {
            return Some(Err(TEXT_HELP));
        }
        Some(Ok(Request {
            source: parts[5].into(),
            destination: parts[7].into(),
            goal: ApplicationGoal::Text {
                find: parts[1].into(),
                replacement: parts[3].into(),
            },
        }))
    } else {
        if parts.len() != 7
            || !parts[2].trim().eq_ignore_ascii_case("to")
            || !parts[4].trim().eq_ignore_ascii_case("and save as")
            || !parts[6].trim().is_empty()
            || !Path::new(parts[1]).is_absolute()
            || !Path::new(parts[5]).is_absolute()
        {
            return Some(Err(CELL_HELP));
        }
        let mut edits = vec![];
        for assignment in parts[3].split(';') {
            let Some((address, value)) = assignment.trim().split_once('=') else {
                return Some(Err(CELL_HELP));
            };
            let Some((row, col)) = coordinate(address.trim()) else {
                return Some(Err(CELL_HELP));
            };
            let value = value.trim();
            let input = if let Some(value) = value.strip_prefix("text:") {
                CellInput::Text {
                    value: value.into(),
                }
            } else if let Some(custodian_winsec::excel::CellValue::Sum {
                first_row,
                first_col,
                last_row,
                last_col,
            }) = custodian_winsec::excel::parse_sum(&format!("={value}"))
            {
                CellInput::Sum {
                    first_row,
                    first_col,
                    last_row,
                    last_col,
                }
            } else if value.parse::<f64>().is_ok() {
                CellInput::Number {
                    decimal: value.into(),
                }
            } else {
                CellInput::Text {
                    value: value.into(),
                }
            };
            edits.push(CellEdit {
                row,
                col,
                value: input,
            });
        }
        if custodian_core::app_adapter::validate_cell_edits("native-pending-sheet", &edits).is_err()
        {
            return Some(Err(CELL_HELP));
        }
        Some(Ok(Request {
            source: parts[1].into(),
            destination: parts[5].into(),
            goal: ApplicationGoal::Cells { edits },
        }))
    }
}
fn executable(name: &str) -> Option<PathBuf> {
    let key = format!(r"Software\Microsoft\Windows\CurrentVersion\App Paths\{name}");
    [Hive::CurrentUser, Hive::LocalMachine]
        .into_iter()
        .filter_map(|hive| registry::string_value(hive, &key, ""))
        .map(|s| PathBuf::from(s.trim_matches('"')))
        .find(|p| p.is_absolute() && p.is_file())
}
fn runtime(app: &AppHandle) -> Result<(PathBuf, PathBuf), String> {
    if cfg!(debug_assertions) {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .map_err(|_| "Development runtime unavailable")?;
        Ok((
            root.join("services/editor-adapter"),
            root.join("target/debug/custodian-appworker.exe"),
        ))
    } else {
        let root = app
            .path()
            .resource_dir()
            .map_err(|_| "Packaged runtime unavailable")?
            .join("runtime");
        Ok((
            root.join("editor-adapter"),
            root.join("bin/custodian-appworker.exe"),
        ))
    }
}
pub(super) fn handle(
    app: &AppHandle,
    state: &AssistantState,
    message: &str,
    channel: Sink<'_>,
) -> bool {
    if let Some(id) = message.trim().strip_prefix("inspect application task ") {
        let result = inspect_task(app, state, id.trim());
        say(channel, result.unwrap_or_else(|_| "Application recovery inspection is unavailable under the current privacy rules or while this task is running. No action was replayed.".into()));
        return true;
    }
    let Some(request) = parse(message) else {
        return false;
    };
    let epoch = state.request_epoch();
    let result = (|| -> Result<ActionCard, String> {
        let request = request.map_err(str::to_string)?;
        let (extension, worker) = runtime(app)?;
        let (program, exe) = match &request.goal {
            ApplicationGoal::Text { .. } => {
                let exe =
                    executable("Code.exe").ok_or("Qualified VS Code installation was not found")?;
                let program = EditorProgram::verify(&exe, &extension).map_err(|e| e.detail)?;
                let exe = program.executable();
                (Program::Editor(program), exe)
            }
            ApplicationGoal::Cells { .. } => {
                let exe =
                    executable("excel.exe").ok_or("Qualified Excel installation was not found")?;
                let program = if cfg!(debug_assertions) {
                    let hash = custodian_core::hashing::hash_file(&worker)
                        .map_err(|_| "Development app worker unavailable")?;
                    ExcelProgram::verify(&worker, &hash, &exe)
                } else {
                    let manifest_path = worker
                        .parent()
                        .and_then(Path::parent)
                        .ok_or("Packaged runtime unavailable")?
                        .join("runtime-manifest.json");
                    let mut bytes = vec![];
                    use std::io::Read;
                    std::fs::File::open(manifest_path)
                        .map_err(|_| "Packaged runtime manifest unavailable")?
                        .take(65537)
                        .read_to_end(&mut bytes)
                        .map_err(|_| "Packaged runtime manifest unavailable")?;
                    if bytes.len() > 65536 {
                        return Err("Packaged runtime manifest exceeds its limit".into());
                    }
                    let manifest: serde_json::Value = serde_json::from_slice(&bytes)
                        .map_err(|_| "Packaged runtime manifest invalid")?;
                    let hash = manifest["applicationWorker"]["sha256"]
                        .as_str()
                        .ok_or("Packaged worker pin missing")?;
                    ExcelProgram::verify_packaged(&worker, hash, &exe)
                }
                .map_err(|e| e.detail)?;
                let exe = program.executable().into();
                (Program::Excel(program), exe)
            }
        };
        let plan = ApplicationPlan::prepare(
            &request.source,
            &request.destination,
            request.goal,
            &exe,
            &state.app_privacy,
        )
        .map_err(|e| e.detail)?;
        let id = format!("app-{}", uuid_id());
        let card = ActionCard { id: id.clone(), title: "Run this local application task?".into(),
            detail: format!("App: {}\nSource: {}\nNew output: {}\nWork in a dedicated application instance and a task folder beside the output. Preserve the original. Verify the saved output and normal app close before completion. Retain the task folder for recovery and verified undo. This approval covers only this task.",
                reply::safe_name(&exe), reply::safe_name(&plan.source.display().to_string()), reply::safe_name(&plan.destination.display().to_string())), confirm: "Run task".into() };
        let now = Instant::now();
        let mut entries = state
            .app_tasks
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        entries.retain(|_, p| now.saturating_duration_since(p.created) < actions::CARD_LIFETIME);
        if entries.len() >= 16 {
            return Err("Too many application approvals are pending. Stop to clear them.".into());
        }
        if state.request_epoch() != epoch
            || state.cancel.load(Ordering::Acquire)
            || !custodian_core::privacy::request_is_current()
        {
            return Err(STOPPED.into());
        }
        entries.insert(
            id,
            Prepared {
                plan,
                program,
                created: now,
                file_revision: state.privacy.revision(),
                epoch,
            },
        );
        Ok(card)
    })();
    match result {
        Ok(card) => emit(channel, ChatEvent::Confirm { card }),
        Err(error) => say(
            channel,
            format!("Application task was not started: {error}"),
        ),
    }
    true
}

fn inspect_task(app: &AppHandle, state: &AssistantState, id: &str) -> Result<String, String> {
    let task = state
        .coordinator
        .get_task(id)
        .ok_or("Application task unavailable")?;
    if task.state.is_active() || !super::task_is_visible(&state.privacy, &task) {
        return Err("Task is active or private".into());
    }
    let binding = task.application.ok_or("No native application binding")?;
    let revision = state
        .app_privacy
        .admit_document_source(&binding.executable, &binding.document_id)
        .map_err(|e| e.detail)?;
    if binding.identity.as_ref().is_some_and(|identity| {
        !state
            .app_privacy
            .task_visible(identity, &binding.document_id)
    }) {
        return Err("Application task is private".into());
    }
    let workspace = binding.workspace.ok_or("No native recovery workspace")?;
    if !task
        .resources
        .iter()
        .any(|resource| Path::new(resource) == workspace)
    {
        return Err("Recovery binding changed".into());
    }
    let files = FileTasks::new(
        crate::paths::data_dir(app)
            .map_err(|_| "Task receipts unavailable")?
            .join("file-tasks"),
    );
    let result = custodian_core::app_recovery::inspect(&workspace, &binding.document_id, &files)
        .map_err(|_| "Recovery evidence requires manual review")?;
    if !custodian_core::privacy::request_is_current()
        || state
            .app_privacy
            .admit_document_source(&binding.executable, &binding.document_id)
            .map_err(|e| e.detail)?
            != revision
    {
        return Err("Privacy authority changed".into());
    }
    Ok(result)
}
fn uuid_id() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub(super) fn decide(
    app: &AppHandle,
    state: &AssistantState,
    id: &str,
    allow: bool,
) -> Result<String, String> {
    let prepared = state.app_tasks.take(id).ok_or(CARD_GONE)?;
    if !allow {
        return Ok("Application task declined.".into());
    }
    if prepared.created.elapsed() >= actions::CARD_LIFETIME
        || prepared.file_revision != state.privacy.revision()
        || prepared.epoch != state.request_epoch()
        || !custodian_core::privacy::request_is_current()
    {
        return Err(
            "This application approval expired or its privacy rules changed. Ask again.".into(),
        );
    }
    prepared
        .plan
        .validate(&state.app_privacy)
        .map_err(|e| e.detail)?;
    let (task, cancel) = state.create_task(
        "Editing and verifying a bound application document".into(),
        TaskState::Preparing,
        &[
            prepared.plan.source.clone(),
            prepared.plan.destination.clone(),
            prepared.plan.workspace.clone(),
        ],
    )?;
    let files = FileTasks::new(
        crate::paths::data_dir(app)
            .map_err(|e| e.to_string())?
            .join("file-tasks"),
    );
    let programs = match &prepared.program {
        Program::Editor(p) => Programs::Editor(p),
        Program::Excel(p) => Programs::Excel(p),
    };
    let result = execute_after_native_consent(
        &prepared.plan,
        programs,
        &state.app_privacy,
        &state.coordinator,
        &task,
        &files,
        &cancel,
    );
    match result {
        Ok(_) => {
            if prepared.epoch != state.request_epoch()
                || prepared.file_revision != state.privacy.revision()
                || state
                    .app_privacy
                    .admit_document_source(&prepared.plan.executable, &prepared.plan.document_id)
                    .is_err()
            {
                return Err("Task evidence was retained. Privacy or Stop changed; inspect current task state before continuing.".into());
            }
            Ok(format!("Application task completed. The saved output and normal app close were independently verified. Original preserved.\nOutput: {}\nVerified undo is available.", prepared.plan.destination.display()))
        }
        Err(error) => {
            if state
                .coordinator
                .get_task(&task)
                .is_some_and(|t| t.state.is_active())
            {
                state
                    .coordinator
                    .update_state(
                        &task,
                        TaskState::RecoveryRequired,
                        Some("Review application task evidence; no automatic replay".into()),
                    )
                    .ok();
            }
            Err(format!("Application task did not complete: {}. Saved effects may remain in the task folder. No action will be replayed automatically.", error.detail))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_only_explicit_bound_goals_and_never_promotes_a_formula_string() {
        assert!(parse(
            r#"Replace "old" with "new" in "C:\work\a.txt" and save as "D:\work\b.txt""#
        )
        .unwrap()
        .is_ok());
        assert!(parse(r#"Replace "old" with "new" in "C:\work\a.txt" and save as "D:\work\b.txt" and delete originals"#).unwrap().is_err());
        let parsed = parse(r#"Set cells in "C:\work\a.xlsx" to "A1=12; A2=text:=RUN(); A3=SUM(A1:A2)" and save as "D:\work\b.xlsx""#).unwrap().unwrap();
        let ApplicationGoal::Cells { edits } = parsed.goal else {
            panic!("cells required")
        };
        assert!(matches!(&edits[1].value, CellInput::Text { value } if value == "=RUN()"));
        assert!(matches!(&edits[2].value, CellInput::Sum { .. }));
        assert!(parse("run a shell command").is_none());
        assert!(
            parse(r#"Set cells in "C:\a.xlsx" to "Q1=7" and save as "D:\b.xlsx""#)
                .unwrap()
                .is_err()
        );
    }
}
