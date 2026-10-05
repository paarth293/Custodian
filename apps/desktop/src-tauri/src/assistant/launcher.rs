//! "Open Spotify", "open my resume": any app in the Start menu, and the
//! person's own documents, photos and media.
//!
//! Apps come only from Windows' own Start menu list and are started the way
//! the Start menu starts them, so Cubby can open exactly what the person
//! could click, and nothing else. Uninstallers and help pages are never
//! matched. Files open with their usual app, and only file types that are
//! documents or media: never programs or scripts.

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// An app in the Start menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartApp {
    pub name: String,
    /// Windows' id for it, used as `shell:AppsFolder\<id>`.
    pub app_id: String,
}

/// How long the Start menu list is reused before being read again.
const CACHE_FOR: Duration = Duration::from_secs(10 * 60);

static CACHE: Mutex<Option<(Instant, Vec<StartApp>)>> = Mutex::new(None);

/// Entries that are not the app itself.
const NOT_APPS: &[&str] = &[
    "uninstall",
    "remove",
    "readme",
    "read me",
    "help",
    "documentation",
    "release notes",
    "website",
    "license",
    "manual",
];

/// Reads Windows' Start menu list (`Get-StartApps`) as JSON.
fn parse_start_apps(json: &str) -> Vec<StartApp> {
    let value: serde_json::Value = match serde_json::from_str(json.trim()) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let items = match value {
        serde_json::Value::Array(items) => items,
        single @ serde_json::Value::Object(_) => vec![single],
        _ => return Vec::new(),
    };
    let mut apps: Vec<StartApp> = Vec::new();
    for item in items {
        let (Some(name), Some(app_id)) = (
            item.get("Name").and_then(|v| v.as_str()),
            item.get("AppID").and_then(|v| v.as_str()),
        ) else {
            continue;
        };
        let lower = name.to_lowercase();
        if name.trim().is_empty()
            || app_id.trim().is_empty()
            || NOT_APPS.iter().any(|w| lower.contains(w))
            || apps.iter().any(|a| a.name.eq_ignore_ascii_case(name))
        {
            continue;
        }
        apps.push(StartApp {
            name: name.trim().to_string(),
            app_id: app_id.trim().to_string(),
        });
    }
    apps
}

#[cfg(windows)]
fn load() -> Vec<StartApp> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // A fixed command: nothing from the person or the model is part of it.
    let output = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Console]::OutputEncoding = [Text.Encoding]::UTF8; Get-StartApps | Select-Object Name, AppID | ConvertTo-Json -Compress",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    match output {
        Ok(out) if out.status.success() => parse_start_apps(&String::from_utf8_lossy(&out.stdout)),
        _ => Vec::new(),
    }
}

#[cfg(not(windows))]
fn load() -> Vec<StartApp> {
    Vec::new()
}

/// The Start menu's apps, read at most every ten minutes.
pub fn start_apps() -> Vec<StartApp> {
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((when, apps)) = cache.as_ref() {
        if when.elapsed() < CACHE_FOR && !apps.is_empty() {
            return apps.clone();
        }
    }
    let apps = load();
    *cache = Some((Instant::now(), apps.clone()));
    apps
}

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// The apps a name means: the app with exactly that name, else apps whose
/// name has all of its words, else apps whose name contains it.
pub fn find<'a>(apps: &'a [StartApp], query: &str) -> Vec<&'a StartApp> {
    let query = query.trim();
    let wanted = words(query);
    if wanted.is_empty() {
        return Vec::new();
    }
    let exact: Vec<&StartApp> = apps
        .iter()
        .filter(|a| a.name.eq_ignore_ascii_case(query))
        .collect();
    if !exact.is_empty() {
        return exact;
    }
    let by_words: Vec<&StartApp> = apps
        .iter()
        .filter(|a| {
            let name = words(&a.name);
            wanted.iter().all(|w| name.contains(w))
        })
        .collect();
    if !by_words.is_empty() {
        return by_words;
    }
    let lower = query.to_lowercase();
    if lower.chars().count() < 3 {
        return Vec::new();
    }
    apps.iter()
        .filter(|a| a.name.to_lowercase().contains(&lower))
        .collect()
}

/// Starts `app` the way the Start menu does.
#[cfg(windows)]
pub fn launch(app: &StartApp) -> std::io::Result<()> {
    std::process::Command::new("explorer.exe")
        .arg(format!("shell:AppsFolder\\{}", app.app_id))
        .spawn()
        .map(|_| ())
}

#[cfg(not(windows))]
pub fn launch(_app: &StartApp) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "opening apps is supported on Windows",
    ))
}

/// File types Cubby may open with their usual app: documents and media,
/// never programs, scripts, shortcuts or web pages.
const OPENABLE: &[&str] = &[
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "txt", "csv",
    "md", "epub", "jpg", "jpeg", "png", "gif", "bmp", "webp", "heic", "tif", "tiff", "mp3", "wav",
    "m4a", "flac", "aac", "ogg", "wma", "mp4", "mkv", "avi", "mov", "wmv", "webm", "m4v", "zip",
];

/// Whether Cubby may open `path` with its usual app.
pub fn openable(path: &Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|e| OPENABLE.contains(&e.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[
        {"Name":"Spotify","AppID":"SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"},
        {"Name":"Visual Studio Code","AppID":"Microsoft.VisualStudioCode"},
        {"Name":"Calendar","AppID":"microsoft.windowscommunicationsapps!calendar"},
        {"Name":"Calendar","AppID":"Microsoft.M365Companions!Calendar"},
        {"Name":"Uninstall VLC","AppID":"{7C5A}\\VLC\\uninstall.exe"},
        {"Name":"VLC media player","AppID":"{7C5A}\\VideoLAN\\VLC\\vlc.exe"},
        {"Name":"VLC Help","AppID":"https://www.videolan.org"},
        {"Name":"","AppID":"x"},
        {"Name":"Calculator","AppID":"Microsoft.WindowsCalculator!App"}
    ]"#;

    #[test]
    fn reads_the_start_menu_without_uninstallers_or_help() {
        let apps = parse_start_apps(SAMPLE);
        let names: Vec<&str> = apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "Spotify",
                "Visual Studio Code",
                "Calendar",
                "VLC media player",
                "Calculator"
            ]
        );
        // One app on its own comes as an object, not a list.
        assert_eq!(
            parse_start_apps(r#"{"Name":"Paint","AppID":"Microsoft.Paint!App"}"#).len(),
            1
        );
        assert!(parse_start_apps("not json").is_empty());
    }

    #[test]
    fn finds_apps_by_name_words_or_part() {
        let apps = parse_start_apps(SAMPLE);
        let names = |q: &str| {
            find(&apps, q)
                .iter()
                .map(|a| a.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(names("spotify"), vec!["Spotify"]);
        assert_eq!(names("Visual Studio Code"), vec!["Visual Studio Code"]);
        assert_eq!(names("code"), vec!["Visual Studio Code"]);
        assert_eq!(names("vlc"), vec!["VLC media player"]);
        assert_eq!(names("calc"), vec!["Calculator"]);
        assert!(names("uninstall vlc").is_empty(), "never an uninstaller");
        assert!(names("xy").is_empty());
        assert!(names("photoshop").is_empty());
    }

    /// Reads the real Start menu (about a second). Run with
    /// `cargo test -p custodian-desktop -- --ignored real_start_menu`.
    #[test]
    #[ignore = "reads this PC's Start menu"]
    fn real_start_menu_lists_apps() {
        let apps = start_apps();
        assert!(!apps.is_empty());
        assert!(apps.iter().all(|a| !a.app_id.is_empty()));
        eprintln!(
            "{} apps, e.g. {:?}",
            apps.len(),
            apps.iter().take(3).collect::<Vec<_>>()
        );
    }

    #[test]
    fn opens_documents_and_media_but_never_programs() {
        for ok in ["Resume.PDF", "a/b/photo.jpeg", "song.mp3", "notes.txt"] {
            assert!(openable(Path::new(ok)), "{ok}");
        }
        for no in [
            "setup.exe",
            "run.bat",
            "x.ps1",
            "x.vbs",
            "x.lnk",
            "x.html",
            "x.svg",
            "x.js",
            "x.msi",
            "invoice.pdf.exe",
            "noext",
        ] {
            assert!(!openable(Path::new(no)), "{no}");
        }
    }
}
