//! Turns a chat message into a typed intent with deterministic rules.
//!
//! These rules handle the requests Cubby can act on today, reliably and
//! offline. Anything they don't recognise becomes [`Intent::Unknown`], which
//! the local language model answers instead (when one is installed).

/// A well-known user folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KnownFolder {
    Downloads,
    Documents,
    Desktop,
    Pictures,
    Videos,
    Music,
}

impl KnownFolder {
    pub const ALL: [KnownFolder; 6] = [
        KnownFolder::Downloads,
        KnownFolder::Documents,
        KnownFolder::Desktop,
        KnownFolder::Pictures,
        KnownFolder::Videos,
        KnownFolder::Music,
    ];

    pub fn label(self) -> &'static str {
        match self {
            KnownFolder::Downloads => "Downloads",
            KnownFolder::Documents => "Documents",
            KnownFolder::Desktop => "Desktop",
            KnownFolder::Pictures => "Pictures",
            KnownFolder::Videos => "Videos",
            KnownFolder::Music => "Music",
        }
    }

    fn from_word(word: &str) -> Option<Self> {
        Some(match word {
            "downloads" | "download" => KnownFolder::Downloads,
            "documents" | "document" | "docs" => KnownFolder::Documents,
            "desktop" => KnownFolder::Desktop,
            "pictures" | "picture" | "photos" | "photo" | "images" => KnownFolder::Pictures,
            "videos" | "video" | "movies" => KnownFolder::Videos,
            "music" | "songs" => KnownFolder::Music,
            _ => return None,
        })
    }
}

/// Which folders a search covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// All the user's personal folders (what "my system" means here).
    UserFolders,
    Folders(Vec<KnownFolder>),
}

/// Apps Cubby may launch. A fixed allow-list: never an arbitrary command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum App {
    Notepad,
    Calculator,
    Paint,
    FileExplorer,
    Settings,
    TaskManager,
}

impl App {
    pub fn label(self) -> &'static str {
        match self {
            App::Notepad => "Notepad",
            App::Calculator => "Calculator",
            App::Paint => "Paint",
            App::FileExplorer => "File Explorer",
            App::Settings => "Settings",
            App::TaskManager => "Task Manager",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "notepad" => App::Notepad,
            "calculator" | "calc" => App::Calculator,
            "paint" | "mspaint" | "ms paint" => App::Paint,
            "file explorer" | "explorer" | "files" | "my computer" | "this pc" => App::FileExplorer,
            "settings" | "windows settings" | "control panel" => App::Settings,
            "task manager" | "taskmgr" => App::TaskManager,
            _ => return None,
        })
    }
}

/// Well-known websites by name.
const SITES: &[(&str, &str)] = &[
    ("myntra", "https://www.myntra.com"),
    ("amazon", "https://www.amazon.in"),
    ("flipkart", "https://www.flipkart.com"),
    ("youtube", "https://www.youtube.com"),
    ("google", "https://www.google.com"),
    ("gmail", "https://mail.google.com"),
    ("github", "https://github.com"),
    ("netflix", "https://www.netflix.com"),
    ("instagram", "https://www.instagram.com"),
    ("facebook", "https://www.facebook.com"),
    ("twitter", "https://x.com"),
    ("x", "https://x.com"),
    ("linkedin", "https://www.linkedin.com"),
    ("wikipedia", "https://www.wikipedia.org"),
    ("whatsapp", "https://web.whatsapp.com"),
    ("spotify", "https://open.spotify.com"),
    ("reddit", "https://www.reddit.com"),
    ("stackoverflow", "https://stackoverflow.com"),
    ("stack overflow", "https://stackoverflow.com"),
    ("zomato", "https://www.zomato.com"),
    ("swiggy", "https://www.swiggy.com"),
    ("chatgpt", "https://chatgpt.com"),
    ("claude", "https://claude.ai"),
    ("maps", "https://maps.google.com"),
    ("google maps", "https://maps.google.com"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    Greeting,
    Thanks,
    Help,
    WhoAreYou,
    Time,
    Date,
    DiskSpace,
    FindDuplicates(Scope),
    LargestFiles(Scope),
    OpenWebsite {
        name: String,
        url: String,
    },
    OpenApp(App),
    OpenFolder(KnownFolder),
    WebSearch(String),
    Undo,
    History,
    Sleep,
    /// "open something" that matched no app, folder or site.
    OpenUnknown(String),
    Unknown,
}

fn normalise(text: &str) -> String {
    let lower = text.trim().to_lowercase();
    let cleaned: String = lower
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " .-/:'".contains(c) {
                c
            } else {
                ' '
            }
        })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn has_any(text: &str, words: &[&str]) -> bool {
    words.iter().any(|w| contains_word(text, w))
}

/// Whole-word (or whole-phrase) match.
fn contains_word(text: &str, phrase: &str) -> bool {
    let bytes = text.as_bytes();
    text.match_indices(phrase).any(|(i, _)| {
        let before = i == 0 || bytes[i - 1] == b' ';
        let end = i + phrase.len();
        let after = end == text.len() || bytes[end] == b' ';
        before && after
    })
}

fn scope_of(text: &str) -> Scope {
    let folders: Vec<KnownFolder> = text.split(' ').filter_map(KnownFolder::from_word).collect();
    if folders.is_empty() {
        Scope::UserFolders
    } else {
        let mut unique = Vec::new();
        for f in folders {
            if !unique.contains(&f) {
                unique.push(f);
            }
        }
        Scope::Folders(unique)
    }
}

/// A safe https URL for a domain-looking target such as "myntra.com".
fn url_for_domain(target: &str) -> Option<String> {
    let host = target
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    let valid = host.contains('.')
        && !host.starts_with('.')
        && !host.ends_with('.')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '/');
    valid.then(|| format!("https://{host}"))
}

const OPEN_VERBS: &[&str] = &[
    "open ",
    "launch ",
    "start ",
    "go to ",
    "goto ",
    "visit ",
    "show me ",
    "take me to ",
];

fn strip_open(text: &str) -> Option<&str> {
    let rest = OPEN_VERBS.iter().find_map(|v| text.strip_prefix(v))?;
    let rest = rest.strip_prefix("the ").unwrap_or(rest);
    let rest = rest.strip_prefix("my ").unwrap_or(rest);
    Some(rest.trim())
}

fn open_target(target: &str) -> Intent {
    let target = target
        .trim_end_matches(" please")
        .trim_end_matches(" for me")
        .trim_end_matches(" app")
        .trim_end_matches(" website")
        .trim_end_matches(" site")
        .trim();
    let folder_name = target.trim_end_matches(" folder").trim();
    if let Some(app) = App::from_name(target) {
        return Intent::OpenApp(app);
    }
    if let Some(folder) = KnownFolder::from_word(folder_name) {
        return Intent::OpenFolder(folder);
    }
    if let Some((name, url)) = SITES.iter().find(|(name, _)| *name == target) {
        return Intent::OpenWebsite {
            name: name.to_string(),
            url: url.to_string(),
        };
    }
    if let Some(url) = url_for_domain(target) {
        return Intent::OpenWebsite {
            name: target.to_string(),
            url,
        };
    }
    Intent::OpenUnknown(target.to_string())
}

/// Classifies a chat message.
pub fn parse(message: &str) -> Intent {
    let text = normalise(message);
    if text.is_empty() {
        return Intent::Unknown;
    }

    if has_any(
        &text,
        &[
            "duplicate",
            "duplicates",
            "duplicated",
            "same files",
            "identical files",
            "copies of",
        ],
    ) {
        return Intent::FindDuplicates(scope_of(&text));
    }
    if has_any(
        &text,
        &[
            "largest",
            "biggest",
            "big files",
            "large files",
            "huge files",
            "taking up space",
            "taking space",
            "eating space",
        ],
    ) {
        return Intent::LargestFiles(scope_of(&text));
    }
    if has_any(
        &text,
        &[
            "disk space",
            "free space",
            "storage",
            "space left",
            "how much space",
            "drive space",
            "disk usage",
        ],
    ) {
        return Intent::DiskSpace;
    }
    if has_any(
        &text,
        &["undo", "bring back", "restore", "put back", "revert"],
    ) {
        return Intent::Undo;
    }
    if has_any(
        &text,
        &[
            "history",
            "what did you do",
            "what have you done",
            "recent changes",
            "show changes",
        ],
    ) {
        return Intent::History;
    }
    for prefix in [
        "search for ",
        "search ",
        "google ",
        "look up ",
        "find online ",
    ] {
        if let Some(query) = text.strip_prefix(prefix) {
            if !query.is_empty() {
                return Intent::WebSearch(query.to_string());
            }
        }
    }
    if has_any(&text, &["go to sleep", "take a nap", "sleep now", "nap"]) {
        return Intent::Sleep;
    }
    if let Some(target) = strip_open(&text) {
        if !target.is_empty() {
            return open_target(target);
        }
    }
    if has_any(
        &text,
        &["what time", "the time", "time is it", "current time"],
    ) {
        return Intent::Time;
    }
    if has_any(
        &text,
        &[
            "what date",
            "the date",
            "today's date",
            "what day",
            "which day",
        ],
    ) {
        return Intent::Date;
    }
    if has_any(
        &text,
        &["help", "what can you do", "commands", "what do you do"],
    ) {
        return Intent::Help;
    }
    if has_any(&text, &["who are you", "your name", "what are you"]) {
        return Intent::WhoAreYou;
    }
    if has_any(&text, &["thanks", "thank you", "thx", "ty"]) {
        return Intent::Thanks;
    }
    let greeting = [
        "hi",
        "hii",
        "hiii",
        "hello",
        "hey",
        "heya",
        "yo",
        "namaste",
        "good morning",
        "good afternoon",
        "good evening",
        "hola",
    ];
    if text.split(' ').count() <= 4 && has_any(&text, &greeting) {
        return Intent::Greeting;
    }
    Intent::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn understands_the_requests_from_the_screenshot() {
        assert_eq!(parse("hii"), Intent::Greeting);
        assert_eq!(
            parse("open myntra"),
            Intent::OpenWebsite {
                name: "myntra".into(),
                url: "https://www.myntra.com".into()
            }
        );
        assert_eq!(
            parse("find the duplicate files in my system"),
            Intent::FindDuplicates(Scope::UserFolders)
        );
    }

    #[test]
    fn understands_the_documented_examples() {
        assert_eq!(
            parse("find duplicates"),
            Intent::FindDuplicates(Scope::UserFolders)
        );
        assert_eq!(
            parse("duplicates in Pictures"),
            Intent::FindDuplicates(Scope::Folders(vec![KnownFolder::Pictures]))
        );
        assert_eq!(parse("undo"), Intent::Undo);
        assert_eq!(parse("what did you do?"), Intent::History);
        assert_eq!(
            parse("biggest files in Downloads"),
            Intent::LargestFiles(Scope::Folders(vec![KnownFolder::Downloads]))
        );
        assert_eq!(parse("how much space?"), Intent::DiskSpace);
        assert_eq!(parse("how much free space?"), Intent::DiskSpace);
        assert_eq!(parse("What can you do?"), Intent::Help);
        assert_eq!(
            parse("open downloads"),
            Intent::OpenFolder(KnownFolder::Downloads)
        );
        assert_eq!(
            parse("search cheap flights"),
            Intent::WebSearch("cheap flights".into())
        );
        assert_eq!(parse("what time is it?"), Intent::Time);
    }

    #[test]
    fn scopes_searches_to_named_folders() {
        assert_eq!(
            parse("Any duplicates in Downloads and my Pictures?"),
            Intent::FindDuplicates(Scope::Folders(vec![
                KnownFolder::Downloads,
                KnownFolder::Pictures
            ]))
        );
        assert_eq!(
            parse("show the biggest files on desktop"),
            Intent::LargestFiles(Scope::Folders(vec![KnownFolder::Desktop]))
        );
    }

    #[test]
    fn opens_apps_folders_sites_and_domains() {
        assert_eq!(parse("open notepad"), Intent::OpenApp(App::Notepad));
        assert_eq!(
            parse("Launch the calculator please"),
            Intent::OpenApp(App::Calculator)
        );
        assert_eq!(
            parse("open my downloads folder"),
            Intent::OpenFolder(KnownFolder::Downloads)
        );
        assert_eq!(
            parse("go to example.org"),
            Intent::OpenWebsite {
                name: "example.org".into(),
                url: "https://example.org".into()
            }
        );
        assert_eq!(
            parse("open frobnicator"),
            Intent::OpenUnknown("frobnicator".into())
        );
    }

    #[test]
    fn rejects_unsafe_looking_domains() {
        assert_eq!(url_for_domain("evil.com\"&calc"), None);
        assert_eq!(url_for_domain("no-dot"), None);
        assert_eq!(url_for_domain(".hidden"), None);
    }

    #[test]
    fn recognises_everyday_requests() {
        assert_eq!(parse("how much free space do I have?"), Intent::DiskSpace);
        assert_eq!(parse("undo that"), Intent::Undo);
        assert_eq!(parse("what did you do"), Intent::History);
        assert_eq!(
            parse("search for cheap flights"),
            Intent::WebSearch("cheap flights".into())
        );
        assert_eq!(parse("what time is it"), Intent::Time);
        assert_eq!(parse("thank you!"), Intent::Thanks);
        assert_eq!(parse("what can you do"), Intent::Help);
        assert_eq!(parse("go to sleep"), Intent::Sleep);
    }

    #[test]
    fn leaves_open_ended_questions_to_the_language_model() {
        assert_eq!(parse("explain how black holes form"), Intent::Unknown);
        assert_eq!(
            parse("hi, can you write me a poem about rain and the sea"),
            Intent::Unknown
        );
    }

    #[test]
    fn matches_whole_words_only() {
        // "this" contains "hi", "storage" is not inside "storages-ish" words.
        assert_eq!(parse("this is great"), Intent::Unknown);
    }
}
