//! "Close Notepad", "what's open?": the apps on the desktop, by the names
//! people use for them. Pure functions over the window list, so the
//! matching is testable without a desktop.

use super::system::friendly_name;
use custodian_winsec::apps::AppWindow;

/// An app and those of its windows that a request is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenApp {
    pub name: String,
    pub windows: Vec<AppWindow>,
}

/// Store apps (Calculator, Settings, Photos, …) run inside this host, so
/// their window title is the only name they have.
const STORE_APP_HOST: &str = "applicationframehost.exe";

/// The name a person uses for the app that owns `window`.
pub fn app_name(window: &AppWindow) -> String {
    if window.exe.is_empty() || window.exe.eq_ignore_ascii_case(STORE_APP_HOST) {
        return window.title.trim().to_string();
    }
    friendly_name(&window.exe).unwrap_or_else(|| window.title.trim().to_string())
}

/// Windows grouped by app, in the order the apps first appear (the
/// frontmost first).
pub fn group(windows: &[AppWindow]) -> Vec<OpenApp> {
    let mut apps: Vec<OpenApp> = Vec::new();
    for window in windows {
        let name = app_name(window);
        match apps.iter_mut().find(|a| a.name.eq_ignore_ascii_case(&name)) {
            Some(app) => app.windows.push(window.clone()),
            None => apps.push(OpenApp {
                name,
                windows: vec![window.clone()],
            }),
        }
    }
    apps
}

/// The words of a request ("the Chrome browser" → "chrome").
fn normalise(query: &str) -> String {
    let mut q = query.trim().to_lowercase();
    for prefix in ["the ", "my ", "this "] {
        if let Some(rest) = q.strip_prefix(prefix) {
            q = rest.to_string();
        }
    }
    for suffix in [
        " app", " apps", " window", " windows", " program", " browser", " please",
    ] {
        if let Some(rest) = q.strip_suffix(suffix) {
            q = rest.to_string();
        }
    }
    q.trim().to_string()
}

/// Requests for closing everything at once, which Cubby never does: one app
/// at a time, so nothing is lost by surprise.
pub fn means_everything(query: &str) -> bool {
    let q = normalise(query);
    matches!(
        q.as_str(),
        "all" | "everything" | "all of them" | "all open" | "everything open" | "all the"
    ) || q.starts_with("all ")
}

fn contains_word(text: &str, word: &str) -> bool {
    text.split(|c: char| !c.is_alphanumeric())
        .collect::<Vec<_>>()
        .windows(word.split_whitespace().count().max(1))
        .any(|w| w.join(" ") == word)
}

/// The apps, and their windows, that a request like "close chrome" means:
/// an app with that name, else an app whose name has that word, else the
/// windows whose title mentions it ("close my resume" means the window
/// showing resume.docx, not every Word window).
pub fn find(windows: &[AppWindow], query: &str) -> Vec<OpenApp> {
    let q = normalise(query);
    if q.is_empty() {
        return Vec::new();
    }
    let apps = group(windows);
    let by_name: Vec<OpenApp> = apps
        .iter()
        .filter(|a| a.name.to_lowercase() == q)
        .cloned()
        .collect();
    if !by_name.is_empty() {
        return by_name;
    }
    let by_word: Vec<OpenApp> = apps
        .iter()
        .filter(|a| contains_word(&a.name.to_lowercase(), &q))
        .cloned()
        .collect();
    if !by_word.is_empty() {
        return by_word;
    }
    let titled: Vec<AppWindow> = windows
        .iter()
        .filter(|w| w.title.to_lowercase().contains(&q))
        .cloned()
        .collect();
    group(&titled)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(handle: isize, exe: &str, title: &str) -> AppWindow {
        AppWindow {
            handle,
            pid: handle as u32,
            title: title.into(),
            exe: exe.into(),
        }
    }

    fn desktop() -> Vec<AppWindow> {
        vec![
            window(1, "chrome.exe", "Inbox - Gmail - Google Chrome"),
            window(2, "WINWORD.EXE", "Resume.docx - Word"),
            window(3, "chrome.exe", "New Tab - Google Chrome"),
            window(4, "ApplicationFrameHost.exe", "Calculator"),
            window(5, "explorer.exe", "Downloads"),
            window(6, "WINWORD.EXE", "Budget notes.docx - Word"),
            window(7, "notepad.exe", "Untitled - Notepad"),
        ]
    }

    fn names(apps: &[OpenApp]) -> Vec<(&str, usize)> {
        apps.iter()
            .map(|a| (a.name.as_str(), a.windows.len()))
            .collect()
    }

    #[test]
    fn groups_windows_by_the_app_people_know() {
        assert_eq!(
            names(&group(&desktop())),
            vec![
                ("Chrome", 2),
                ("Word", 2),
                ("Calculator", 1),
                ("File Explorer", 1),
                ("Notepad", 1)
            ]
        );
    }

    #[test]
    fn finds_apps_by_name_word_or_window_title() {
        let all = desktop();
        assert_eq!(names(&find(&all, "chrome")), vec![("Chrome", 2)]);
        assert_eq!(
            names(&find(&all, "the Chrome browser")),
            vec![("Chrome", 2)]
        );
        assert_eq!(names(&find(&all, "google chrome")), vec![("Chrome", 2)]);
        assert_eq!(names(&find(&all, "calculator")), vec![("Calculator", 1)]);
        assert_eq!(names(&find(&all, "explorer")), vec![("File Explorer", 1)]);
        assert_eq!(names(&find(&all, "Notepad")), vec![("Notepad", 1)]);
        // A document closes its own window, not every Word window.
        let resume = find(&all, "my resume");
        assert_eq!(names(&resume), vec![("Word", 1)]);
        assert_eq!(resume[0].windows[0].handle, 2);
        assert!(find(&all, "photoshop").is_empty());
        assert!(find(&all, "  ").is_empty());
    }

    #[test]
    fn several_matches_are_left_for_the_person_to_choose() {
        // "docx" is in two Word windows: one app, two windows.
        assert_eq!(names(&find(&desktop(), "docx")), vec![("Word", 2)]);
        // "new" is in a Chrome title only.
        assert_eq!(names(&find(&desktop(), "new tab")), vec![("Chrome", 1)]);
    }

    #[test]
    fn never_means_everything_by_accident() {
        for q in [
            "all",
            "everything",
            "all apps",
            "all open apps",
            "all my windows",
        ] {
            assert!(means_everything(q), "{q}");
        }
        for q in ["chrome", "word", "the calculator", "allegro"] {
            assert!(!means_everything(q), "{q}");
        }
    }
}
