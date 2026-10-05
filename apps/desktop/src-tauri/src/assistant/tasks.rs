//! Cubby's to-do list and reminders. The list lives only on this PC, sealed
//! with DPAPI so other Windows accounts, and anyone holding a copy of the
//! disk without this account, cannot read it (D-05, SR-F2).

use super::reply::{count, plural, safe_name};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Longest task text kept.
pub const MAX_TASK_CHARS: usize = 200;
/// Most tasks kept open at once.
pub const MAX_OPEN_TASKS: usize = 500;
/// Finished or removed tasks are forgotten after this long.
const KEEP_FINISHED_SECS: i64 = 90 * 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: u64,
    pub text: String,
    pub created_unix: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_unix: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done_unix: Option<i64>,
    #[serde(default)]
    pub removed: bool,
    /// The reminder has been shown.
    #[serde(default)]
    pub reminded: bool,
}

impl Task {
    fn is_open(&self) -> bool {
        self.done_unix.is_none() && !self.removed
    }
}

/// What can go wrong adding a task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddError {
    Empty,
    Full,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskList {
    next_id: u64,
    tasks: Vec<Task>,
}

impl TaskList {
    /// Adds a task, trimmed to [`MAX_TASK_CHARS`].
    pub fn add(
        &mut self,
        text: &str,
        due_unix: Option<i64>,
        now_unix: i64,
    ) -> Result<Task, AddError> {
        let text: String = text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .filter(|c| !c.is_control())
            .take(MAX_TASK_CHARS)
            .collect();
        if text.is_empty() {
            return Err(AddError::Empty);
        }
        if self.open().len() >= MAX_OPEN_TASKS {
            return Err(AddError::Full);
        }
        self.next_id += 1;
        let task = Task {
            id: self.next_id,
            text,
            created_unix: now_unix,
            due_unix,
            done_unix: None,
            removed: false,
            reminded: false,
        };
        self.tasks.push(task.clone());
        Ok(task)
    }

    /// Open tasks: reminders by due time first, then the rest as added.
    pub fn open(&self) -> Vec<&Task> {
        let mut open: Vec<&Task> = self.tasks.iter().filter(|t| t.is_open()).collect();
        open.sort_by_key(|t| (t.due_unix.is_none(), t.due_unix, t.id));
        open
    }

    /// The open tasks a request means: "2" is the second on the list;
    /// otherwise tasks whose text contains every word of `query`.
    pub fn find(&self, query: &str) -> Vec<&Task> {
        let open = self.open();
        let query = query.trim().trim_start_matches('#');
        if let Ok(n) = query.parse::<usize>() {
            return open.get(n.wrapping_sub(1)).copied().into_iter().collect();
        }
        let words: Vec<String> = query
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(str::to_lowercase)
            .collect();
        if words.is_empty() {
            return Vec::new();
        }
        let matches: Vec<&Task> = open
            .iter()
            .copied()
            .filter(|t| {
                let text = t.text.to_lowercase();
                words.iter().all(|w| text.contains(w.as_str()))
            })
            .collect();
        // An exact match wins over longer tasks that also contain the words.
        let exact: Vec<&Task> = matches
            .iter()
            .copied()
            .filter(|t| t.text.eq_ignore_ascii_case(query))
            .collect();
        if exact.len() == 1 {
            exact
        } else {
            matches
        }
    }

    fn get_mut(&mut self, id: u64) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id && t.is_open())
    }

    pub fn complete(&mut self, id: u64, now_unix: i64) -> bool {
        self.get_mut(id)
            .map(|t| t.done_unix = Some(now_unix))
            .is_some()
    }

    pub fn remove(&mut self, id: u64, now_unix: i64) -> bool {
        self.get_mut(id)
            .map(|t| {
                t.removed = true;
                t.done_unix.get_or_insert(now_unix);
            })
            .is_some()
    }

    /// Open reminders due by `now` that were not shown yet. They are marked
    /// shown, so each reminder appears once.
    pub fn take_due(&mut self, now_unix: i64) -> Vec<Task> {
        let mut due = Vec::new();
        for task in self.tasks.iter_mut() {
            if task.is_open() && !task.reminded && task.due_unix.is_some_and(|d| d <= now_unix) {
                task.reminded = true;
                due.push(task.clone());
            }
        }
        due
    }

    /// Forgets finished and removed tasks older than three months.
    fn prune(&mut self, now_unix: i64) {
        self.tasks.retain(|t| {
            t.is_open()
                || t.done_unix
                    .is_some_and(|d| now_unix - d < KEEP_FINISHED_SECS)
        });
    }
}

/// Reads the sealed list at `path`. A missing list is empty; one that cannot
/// be unsealed (for example copied from another account) is set aside as
/// `*.unreadable` rather than overwritten.
pub fn load(path: &Path) -> io::Result<TaskList> {
    let sealed = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(TaskList::default()),
        Err(err) => return Err(err),
    };
    let list = custodian_winsec::dpapi::unprotect(&sealed)
        .ok()
        .and_then(|plain| serde_json::from_slice(&plain).ok());
    match list {
        Some(list) => Ok(list),
        None => {
            fs::rename(path, path.with_extension("unreadable"))?;
            Ok(TaskList::default())
        }
    }
}

/// Seals and writes the list, all at once, so a crash never leaves half a file.
pub fn save(path: &Path, list: &mut TaskList, now_unix: i64) -> io::Result<()> {
    list.prune(now_unix);
    let plain = serde_json::to_vec(list).map_err(io::Error::other)?;
    let sealed = custodian_winsec::dpapi::protect(&plain)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    let mut file = fs::File::create(&tmp)?;
    file.write_all(&sealed)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, path)
}

/// A local time as unix seconds (the earliest, if the clock changes).
pub fn to_unix(local: NaiveDateTime) -> Option<i64> {
    Local
        .from_local_datetime(&local)
        .earliest()
        .map(|t| t.timestamp())
}

/// "today at 5:00 PM", "tomorrow at 9:00 AM", "on Friday at 3:00 PM",
/// "on 12 October at 9:00 AM", or "in 20 minutes" for soon.
pub fn when_words(due_unix: i64, now: DateTime<Local>) -> String {
    let Some(due) = Local.timestamp_opt(due_unix, 0).single() else {
        return "later".into();
    };
    let minutes = (due - now).num_minutes();
    if (0..60).contains(&minutes) {
        let n = minutes.max(1) as usize;
        return format!("in {} {}", count(n), plural(n, "minute", "minutes"));
    }
    let time = due.format("%-I:%M %p");
    match (due.date_naive() - now.date_naive()).num_days() {
        0 => format!("today at {time}"),
        1 => format!("tomorrow at {time}"),
        2..=6 => format!("on {} at {time}", due.format("%A")),
        _ => format!("on {} at {time}", due.format("%-d %B")),
    }
}

/// After adding a task.
pub fn added_text(task: &Task, now: DateTime<Local>) -> String {
    let text = safe_name(&task.text);
    match task.due_unix {
        Some(due) => format!(
            "Okay! I'll remind you {}: \"{text}\".",
            when_words(due, now)
        ),
        None => format!("Added \"{text}\" to your list."),
    }
}

/// The answer to "what's on my list?".
pub fn list_text(list: &TaskList, now: DateTime<Local>) -> String {
    let open = list.open();
    if open.is_empty() {
        return "Your list is empty. Say \"remind me to … at 5pm\" or \"add … to my list\".".into();
    }
    let mut text = format!(
        "You have {} {}:",
        count(open.len()),
        plural(open.len(), "thing on your list", "things on your list")
    );
    for (i, task) in open.iter().enumerate() {
        text.push_str(&format!("\n{}. {}", i + 1, safe_name(&task.text)));
        if let Some(due) = task.due_unix {
            text.push_str(&format!(" — {}", when_words(due, now)));
        }
    }
    text.push_str("\n\nSay \"mark 1 as done\" when you finish one.");
    text
}

/// When several tasks match a request.
pub fn which_text(matches: &[&Task]) -> String {
    let names: Vec<String> = matches
        .iter()
        .take(5)
        .map(|t| format!("\"{}\"", safe_name(&t.text)))
        .collect();
    format!(
        "Which one do you mean: {}? You can also use its number from your list.",
        names.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> TaskList {
        let mut list = TaskList::default();
        list.add("Buy milk", None, 100).unwrap();
        list.add("Call mom", Some(5_000), 100).unwrap();
        list.add("Buy birthday present for Sam", None, 100).unwrap();
        list
    }

    #[test]
    fn lists_reminders_first_then_the_rest_in_order() {
        let list = list();
        let texts: Vec<&str> = list.open().iter().map(|t| t.text.as_str()).collect();
        assert_eq!(
            texts,
            vec!["Call mom", "Buy milk", "Buy birthday present for Sam"]
        );
    }

    #[test]
    fn finds_tasks_by_words_or_number() {
        let list = list();
        assert_eq!(list.find("milk")[0].text, "Buy milk");
        assert_eq!(list.find("2")[0].text, "Buy milk", "number 2 on the list");
        assert_eq!(list.find("#1")[0].text, "Call mom");
        assert!(list.find("9").is_empty());
        assert!(list.find("0").is_empty());
        assert_eq!(list.find("buy").len(), 2, "ambiguous");
        assert_eq!(list.find("buy milk").len(), 1, "exact wins");
        assert!(list.find("taxes").is_empty());
        assert!(list.find("  ").is_empty());
    }

    #[test]
    fn completes_and_removes_once() {
        let mut list = list();
        let milk = list.find("milk")[0].id;
        assert!(list.complete(milk, 200));
        assert!(!list.complete(milk, 201), "already done");
        assert!(list.find("milk").is_empty());
        let mom = list.find("mom")[0].id;
        assert!(list.remove(mom, 200));
        assert_eq!(list.open().len(), 1);
    }

    #[test]
    fn each_reminder_fires_once_and_only_when_due() {
        let mut list = list();
        assert!(list.take_due(4_999).is_empty());
        let due = list.take_due(5_000);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].text, "Call mom");
        assert!(list.take_due(9_999).is_empty(), "shown once");
        // A finished reminder never fires.
        let mut list = TaskList::default();
        let t = list.add("Stretch", Some(10), 0).unwrap();
        list.complete(t.id, 5);
        assert!(list.take_due(20).is_empty());
    }

    #[test]
    fn keeps_task_text_short_and_printable() {
        let mut list = TaskList::default();
        let t = list
            .add(&format!("water\tplants\n{}", "x".repeat(500)), None, 0)
            .unwrap();
        assert!(t.text.starts_with("water plants "));
        assert_eq!(t.text.chars().count(), MAX_TASK_CHARS);
        assert_eq!(list.add("   ", None, 0), Err(AddError::Empty));
    }

    #[test]
    fn forgets_old_finished_tasks_only() {
        let mut list = list();
        let milk = list.find("milk")[0].id;
        list.complete(milk, 100);
        list.prune(100 + KEEP_FINISHED_SECS + 1);
        assert_eq!(list.tasks.len(), 2);
        assert!(list.open().iter().all(|t| t.text != "Buy milk"));
    }

    #[cfg(windows)]
    #[test]
    fn saves_sealed_and_loads_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tasks.dpapi");
        assert_eq!(load(&path).unwrap(), TaskList::default());
        let mut original = list();
        save(&path, &mut original, 100).unwrap();
        let raw = fs::read(&path).unwrap();
        assert!(
            !raw.windows(8).any(|w| w == b"Call mom"),
            "sealed, not plain text"
        );
        assert_eq!(load(&path).unwrap(), original);

        // A list that cannot be unsealed is set aside, not overwritten.
        fs::write(&path, b"garbage").unwrap();
        assert_eq!(load(&path).unwrap(), TaskList::default());
        assert!(path.with_extension("unreadable").exists());
    }

    #[test]
    fn says_when_in_plain_words() {
        let now = Local.with_ymd_and_hms(2026, 10, 5, 10, 0, 0).unwrap();
        let at = |d: u32, h: u32, m: u32| {
            Local
                .with_ymd_and_hms(2026, 10, d, h, m, 0)
                .unwrap()
                .timestamp()
        };
        assert_eq!(when_words(at(5, 10, 20), now), "in 20 minutes");
        assert_eq!(when_words(at(5, 17, 0), now), "today at 5:00 PM");
        assert_eq!(when_words(at(6, 9, 0), now), "tomorrow at 9:00 AM");
        assert_eq!(when_words(at(9, 15, 0), now), "on Friday at 3:00 PM");
        assert_eq!(when_words(at(20, 9, 0), now), "on 20 October at 9:00 AM");
    }

    #[test]
    fn words_for_adding_and_listing() {
        let now = Local.with_ymd_and_hms(2026, 10, 5, 10, 0, 0).unwrap();
        let mut list = TaskList::default();
        let plain = list.add("Buy milk", None, 0).unwrap();
        assert_eq!(added_text(&plain, now), "Added \"Buy milk\" to your list.");
        let due = now.timestamp() + 7 * 3600;
        let timed = list.add("Call mom", Some(due), 0).unwrap();
        assert_eq!(
            added_text(&timed, now),
            "Okay! I'll remind you today at 5:00 PM: \"Call mom\"."
        );
        let text = list_text(&list, now);
        assert!(
            text.starts_with("You have 2 things on your list:"),
            "{text}"
        );
        assert!(text.contains("1. Call mom — today at 5:00 PM"), "{text}");
        assert!(text.contains("2. Buy milk"), "{text}");
        assert!(list_text(&TaskList::default(), now).starts_with("Your list is empty"));
    }
}
