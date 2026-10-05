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

    /// The folder with this label ("Downloads"), in any case.
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|f| f.label().eq_ignore_ascii_case(label))
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

    /// The app with this tool name ("file_explorer").
    pub fn from_tool_name(name: &str) -> Option<Self> {
        Some(match name {
            "notepad" => App::Notepad,
            "calculator" => App::Calculator,
            "paint" => App::Paint,
            "file_explorer" => App::FileExplorer,
            "settings" => App::Settings,
            "task_manager" => App::TaskManager,
            _ => return None,
        })
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
    /// "Where's my resume?": files whose names contain these words.
    FindFiles {
        name: String,
        scope: Scope,
    },
    /// CPU, memory, battery and uptime ("is my PC slow?").
    SystemStatus,
    /// "Empty the safe place": free the space of quarantined copies now.
    EmptySafePlace,
    /// "What apps are open?"
    ListOpenApps,
    /// "Close Notepad": ask an open app to close, as its X button does.
    CloseApp(String),
    /// "Close yourself": Cubby can't, but can say how to hide.
    CloseCubby,
    /// "Remind me to call mom at 5pm", "add milk to my list": the person's
    /// own words, time included (read by `when`).
    AddTask(String),
    /// "What's on my list?"
    ListTasks,
    /// "Mark milk as done", "done with 2".
    CompleteTask(String),
    /// "Remove milk from my list".
    RemoveTask(String),
    /// "Clean my temp files".
    CleanTempFiles,
    /// "Any suggestions?", "health check": a quick check-up.
    Suggestions,
    /// "Is my data safe?": how Cubby protects it, as it stands on this PC.
    SecurityStatus,
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

/// A well-known site name ("youtube") or a domain ("example.org") as
/// `(name, https URL)`.
pub fn website(target: &str) -> Option<(String, String)> {
    let target = target.trim().to_lowercase();
    if let Some((name, url)) = SITES.iter().find(|(name, _)| *name == target) {
        return Some((name.to_string(), url.to_string()));
    }
    url_for_domain(&target).map(|url| (target, url))
}

/// File extensions that make a name a file, never a website, even where
/// they are also web domains (".zip" and ".mov" are both, a known trick).
const FILE_EXTENSIONS: &[&str] = &[
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "txt", "csv",
    "md", "epub", "jpg", "jpeg", "png", "gif", "bmp", "webp", "heic", "tif", "tiff", "mp3", "wav",
    "m4a", "flac", "aac", "ogg", "wma", "mp4", "mkv", "avi", "mov", "wmv", "webm", "m4v", "zip",
    "rar", "7z", "exe", "msi", "bat", "cmd", "ps1", "lnk", "iso", "json", "xml", "log",
];

/// Whether `target` names a file ("resume.pdf"), not a website.
fn looks_like_file(target: &str) -> bool {
    target
        .rsplit_once('.')
        .is_some_and(|(_, ext)| FILE_EXTENSIONS.contains(&ext.trim_end_matches('/')))
}

/// A safe https URL for a domain-looking target such as "myntra.com".
fn url_for_domain(target: &str) -> Option<String> {
    let host = target
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    let valid = host.contains('.')
        && !looks_like_file(host)
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

/// Verbs that mean "open" and nothing else. With the others ("show me",
/// "start") an unknown target is more likely a question for the model
/// ("show me how to make pasta").
const STRICT_OPEN_VERBS: &[&str] = &["open ", "launch "];

/// The target after an open verb, and whether the verb was a strict one.
fn strip_open(text: &str) -> Option<(&str, bool)> {
    let verb = OPEN_VERBS.iter().find(|v| text.starts_with(**v))?;
    let rest = &text[verb.len()..];
    let rest = rest.strip_prefix("the ").unwrap_or(rest);
    let rest = rest.strip_prefix("my ").unwrap_or(rest);
    Some((rest.trim(), STRICT_OPEN_VERBS.contains(verb)))
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

/// Openings of questions about the world rather than requests to act
/// ("what is the largest planet?", "how do I restore a backup?").
const QUESTION_STARTS: &[&str] = &[
    "explain ",
    "define ",
    "tell me about ",
    "what does ",
    "what do ",
    "what is ",
    "what are ",
    "what's ",
    "whats ",
    "why ",
    "how do ",
    "how does ",
    "how did ",
    "how to ",
    "how can ",
    "how should ",
    "how would ",
    "how long ",
    "is it ",
    "is there ",
    "are there ",
    "should i ",
    "can you explain ",
    "who ",
    "when ",
    "which ",
];

fn is_question(text: &str) -> bool {
    QUESTION_STARTS.iter().any(|q| text.starts_with(q))
}

/// The message is about the person's own PC or files.
fn about_own_stuff(text: &str) -> bool {
    has_any(
        text,
        &[
            "my",
            "mine",
            "i have",
            "do i have",
            "this pc",
            "this computer",
            "this laptop",
        ],
    ) || text.split(' ').any(|w| KnownFolder::from_word(w).is_some())
}

/// The message is about files or storage at all.
fn about_files(text: &str) -> bool {
    about_own_stuff(text)
        || has_any(
            text,
            &[
                "file", "files", "folder", "folders", "storage", "space", "disk", "drive", "pc",
                "computer", "laptop", "system",
            ],
        )
}

/// Whether a message that mentions an action really asks for it. Questions
/// about the world don't ("what is the largest planet?"); questions about
/// the person's own things do ("what are the biggest files in my Downloads?",
/// "what are the duplicate files?").
fn wants_action(text: &str) -> bool {
    !is_question(text)
        || about_own_stuff(text)
        || (contains_word(text, "the")
            && has_any(text, &["files", "file", "folders", "duplicates"]))
}

fn word_count(text: &str) -> usize {
    text.split(' ').count()
}

/// "where's my resume in documents?" gives ("resume", [Documents]).
fn find_target(text: &str) -> Option<(String, Scope)> {
    const PREFIXES: &[&str] = &[
        "where is my ",
        "where's my ",
        "wheres my ",
        "where did i put my ",
        "where did i save my ",
        "find my ",
        "find the file ",
        "find file ",
        "find files named ",
        "find files called ",
        "locate my ",
        "look for my ",
        "search my pc for ",
        "search my computer for ",
        "search my files for ",
        "search for my ",
    ];
    let rest = PREFIXES.iter().find_map(|p| text.strip_prefix(p))?;
    let mut words: Vec<&str> = rest.split(' ').filter(|w| !w.is_empty()).collect();
    let mut scope = Scope::UserFolders;
    // A trailing "in documents" / "on desktop" names the folder to search.
    if words.len() >= 3 && ["in", "on", "from"].contains(&words[words.len() - 2]) {
        if let Some(folder) = KnownFolder::from_word(words[words.len() - 1]) {
            scope = Scope::Folders(vec![folder]);
            words.truncate(words.len() - 2);
        }
    }
    const FILLER: &[&str] = &[
        "the",
        "my",
        "please",
        "file",
        "files",
        "document",
        "documents",
    ];
    while words.first().is_some_and(|w| FILLER.contains(w)) {
        words.remove(0);
    }
    while words
        .last()
        .is_some_and(|w| FILLER.contains(w) || *w == "for" || *w == "me")
    {
        words.pop();
    }
    (!words.is_empty() && words.len() <= 6).then(|| (words.join(" "), scope))
}

/// `text` without the ASCII `prefix`, ignoring case.
fn strip_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.get(..prefix.len())
        .filter(|p| p.eq_ignore_ascii_case(prefix))
        .and_then(|_| text.get(prefix.len()..))
}

/// `text` without the ASCII `suffix`, ignoring case.
fn strip_suffix_ci<'a>(text: &'a str, suffix: &str) -> Option<&'a str> {
    let start = text.len().checked_sub(suffix.len())?;
    text.get(start..)
        .filter(|s| s.eq_ignore_ascii_case(suffix))
        .and_then(|_| text.get(..start))
}

/// Ways of asking for a reminder or a to-do item, followed by the task.
const ADD_TASK_PREFIXES: &[&str] = &[
    "remind me to ",
    "remind me that ",
    "remind me about ",
    "remind me ",
    "set a reminder to ",
    "set a reminder for ",
    "set a reminder ",
    "set reminder to ",
    "set reminder for ",
    "add a reminder to ",
    "add a reminder for ",
    "add reminder to ",
    "create a reminder to ",
    "create a reminder for ",
    "add a task to ",
    "add a task: ",
    "add a task ",
    "add task: ",
    "add task ",
    "new task: ",
    "new task ",
    "create a task to ",
    "create a task ",
    "to-do: ",
    "todo: ",
];

/// "add milk to my list".
const LIST_SUFFIXES: &[&str] = &[
    " to my to-do list",
    " to my todo list",
    " to my to do list",
    " to my task list",
    " to my list",
    " to the list",
    " to my tasks",
    " to my reminders",
    " to the to-do list",
    " to the todo list",
    " to my to-dos",
];

/// "remove milk from my list".
const FROM_LIST_SUFFIXES: &[&str] = &[
    " from my to-do list",
    " from my todo list",
    " from my to do list",
    " from my task list",
    " from my list",
    " from the list",
    " from my tasks",
    " from my reminders",
    " off my list",
    " off the list",
];

/// To-do and reminder requests, keeping the person's own words for the task.
fn task_intent(message: &str, text: &str) -> Option<Intent> {
    let original = message.trim().trim_end_matches(['.', '!']).trim();
    for prefix in ADD_TASK_PREFIXES {
        if let Some(rest) = strip_prefix_ci(original, prefix) {
            if !rest.trim().is_empty() {
                return Some(Intent::AddTask(rest.trim().to_string()));
            }
        }
    }
    if let Some(rest) = strip_prefix_ci(original, "add ") {
        for suffix in LIST_SUFFIXES {
            if let Some(item) = strip_suffix_ci(rest, suffix) {
                if !item.trim().is_empty() {
                    return Some(Intent::AddTask(item.trim().to_string()));
                }
            }
        }
    }
    if wants_task_list(text) {
        return Some(Intent::ListTasks);
    }
    // "mark milk as done", "mark 2 done"
    if let Some(rest) = text.strip_prefix("mark ") {
        for suffix in [
            " as done",
            " as complete",
            " as completed",
            " as finished",
            " done",
            " complete",
            " completed",
            " finished",
        ] {
            if let Some(item) = rest.strip_suffix(suffix) {
                if !item.trim().is_empty() {
                    return Some(Intent::CompleteTask(task_name(item)));
                }
            }
        }
    }
    for prefix in [
        "tick off ",
        "cross off ",
        "check off ",
        "i finished ",
        "i've finished ",
        "ive finished ",
        "i'm done with ",
        "im done with ",
        "done with ",
        "complete the task ",
        "complete task ",
    ] {
        if let Some(item) = text.strip_prefix(prefix) {
            if !item.trim().is_empty() {
                return Some(Intent::CompleteTask(task_name(item)));
            }
        }
    }
    for verb in ["remove ", "delete ", "take "] {
        if let Some(rest) = text.strip_prefix(verb) {
            for suffix in FROM_LIST_SUFFIXES {
                if let Some(item) = rest.strip_suffix(suffix) {
                    if !item.trim().is_empty() {
                        return Some(Intent::RemoveTask(task_name(item)));
                    }
                }
            }
        }
    }
    for prefix in [
        "delete the task ",
        "remove the task ",
        "cancel the task ",
        "delete the reminder ",
        "remove the reminder ",
        "cancel the reminder ",
        "cancel my reminder ",
        "delete my reminder ",
    ] {
        if let Some(item) = text.strip_prefix(prefix) {
            if !item.trim().is_empty() {
                return Some(Intent::RemoveTask(task_name(item)));
            }
        }
    }
    None
}

/// The words naming a task, without "the task", "to" or "about" in front.
fn task_name(item: &str) -> String {
    let mut item = item.trim();
    for prefix in [
        "the task ",
        "task ",
        "the reminder ",
        "reminder ",
        "to ",
        "about ",
        "the ",
    ] {
        item = item.strip_prefix(prefix).unwrap_or(item);
    }
    item.trim().to_string()
}

/// "What's on my list?", "show my reminders", "what do I have to do?".
fn wants_task_list(text: &str) -> bool {
    let t = text.trim_end_matches(['?', '.', '!']).trim();
    if matches!(
        t,
        "what do i have to do"
            | "what do i need to do"
            | "what do i have to do today"
            | "what reminders do i have"
            | "what are my tasks"
            | "what are my reminders"
    ) {
        return true;
    }
    let mut rest = t;
    for prefix in [
        "what's ", "whats ", "what is ", "show me ", "show ", "open ", "list ", "read me ",
        "tell me ", "check ",
    ] {
        if let Some(r) = rest.strip_prefix(prefix) {
            rest = r;
            break;
        }
    }
    matches!(
        rest,
        "on my list"
            | "on my to-do list"
            | "on my todo list"
            | "on my to do list"
            | "my list"
            | "my tasks"
            | "my reminders"
            | "my to-do list"
            | "my todo list"
            | "my to do list"
            | "my task list"
            | "my to-dos"
            | "my todos"
            | "tasks"
            | "reminders"
            | "to-do list"
            | "todo list"
            | "to do list"
    )
}

/// Verbs that mean "close this app". All of them close gracefully.
const CLOSE_VERBS: &[&str] = &["close ", "quit ", "exit ", "kill "];

/// Things "close …" can name that are Cubby itself.
const CUBBY_NAMES: &[&str] = &[
    "cubby",
    "yourself",
    "you",
    "this",
    "chat",
    "the chat",
    "this chat",
    "panel",
    "the panel",
    "chat panel",
    "custodian",
];

/// "close notepad" gives `CloseApp("notepad")`.
fn close_target(text: &str) -> Option<Intent> {
    let verb = CLOSE_VERBS.iter().find(|v| text.starts_with(**v))?;
    let target = text[verb.len()..]
        .trim()
        .trim_end_matches(" please")
        .trim_end_matches(" for me")
        .trim();
    if target.is_empty() || word_count(target) > 4 {
        return None;
    }
    if CUBBY_NAMES.contains(&target) {
        return Some(Intent::CloseCubby);
    }
    Some(Intent::CloseApp(target.to_string()))
}

/// "What apps are open?", "what's running on my pc?".
fn wants_open_apps(text: &str) -> bool {
    if has_any(
        text,
        &[
            "what apps are open",
            "which apps are open",
            "what apps are running",
            "which apps are running",
            "what apps do i have open",
            "what programs are open",
            "what programs are running",
            "what windows are open",
            "list open apps",
            "show open apps",
            "show my open apps",
            "list my open apps",
            "list running apps",
            "show running apps",
        ],
    ) {
        return true;
    }
    // The short forms only when they are clearly about this PC, so that
    // "what's open near me?" stays a question for the model.
    let mut rest = text.trim_end_matches(['?', '.', '!']).trim();
    for tail in [
        " right now",
        " now",
        " on my pc",
        " on my computer",
        " on my laptop",
        " on my desktop",
        " on my screen",
    ] {
        rest = rest.strip_suffix(tail).unwrap_or(rest);
    }
    matches!(
        rest,
        "what's open"
            | "whats open"
            | "what is open"
            | "what's running"
            | "whats running"
            | "what is running"
            | "open apps"
            | "running apps"
    )
}

/// "Any suggestions?", "health check", "how can I free up space?".
fn wants_suggestions(text: &str) -> bool {
    let t = text.trim_end_matches(['?', '.', '!']).trim();
    matches!(
        t,
        "suggestions"
            | "any suggestions"
            | "suggest something"
            | "give me suggestions"
            | "give me some suggestions"
            | "what do you suggest"
            | "what would you suggest"
            | "what should i do"
            | "what should i clean"
            | "what should i delete"
            | "what can i delete"
            | "what can i clean"
            | "health check"
            | "checkup"
            | "check up"
            | "check-up"
            | "pc checkup"
            | "pc check-up"
            | "check my pc"
            | "check my computer"
            | "check my laptop"
            | "check up on my pc"
            | "give my pc a checkup"
            | "give my pc a check-up"
    ) || has_any(
        t,
        &[
            "how can i free up space",
            "how do i free up space",
            "how can i free some space",
            "help me free up space",
            "free up some space",
            "free up space",
            "speed up my pc",
            "speed up my computer",
            "speed up my laptop",
            "make my pc faster",
            "make my computer faster",
            "make my laptop faster",
            "optimize my pc",
            "optimise my pc",
            "optimize my computer",
            "optimise my computer",
            "tune up my pc",
        ],
    )
}

/// "Empty the safe place", "free the space now".
fn wants_empty_safe_place(text: &str) -> bool {
    let place = has_any(text, &["safe place", "safe-place", "quarantine"]);
    let verb = has_any(text, &["empty", "clear", "purge", "free"]);
    (place && verb)
        || has_any(
            text,
            &[
                "free the space now",
                "free up the space now",
                "free that space now",
                "free up that space now",
            ],
        )
}

/// Questions about how the PC is doing ("is my laptop slow?", "battery").
fn about_system(text: &str) -> bool {
    has_any(
        text,
        &[
            "battery",
            "cpu",
            "processor",
            "ram",
            "memory usage",
            "system status",
            "uptime",
            "task manager stats",
        ],
    ) || (has_any(text, &["slow", "laggy", "lagging", "hanging", "freezing"])
        && has_any(text, &["pc", "computer", "laptop", "system", "it"]))
}

/// Classifies a chat message.
pub fn parse(message: &str) -> Intent {
    let text = normalise(message);
    if text.is_empty() {
        return Intent::Unknown;
    }
    // Tasks first: "remind me to put back the chairs" is not an undo.
    if let Some(intent) = task_intent(message, &text) {
        return intent;
    }
    let act = wants_action(&text);
    let words = word_count(&text);

    if act
        && has_any(
            &text,
            &[
                "duplicate",
                "duplicates",
                "duplicated",
                "same files",
                "identical files",
                "copies of",
            ],
        )
        && (about_files(&text) || words <= 3)
    {
        return Intent::FindDuplicates(scope_of(&text));
    }
    if act
        && about_files(&text)
        && has_any(
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
                "using space",
                "using my space",
            ],
        )
    {
        return Intent::LargestFiles(scope_of(&text));
    }
    let disk_phrase = has_any(
        &text,
        &[
            "disk space",
            "free space",
            "space left",
            "drive space",
            "disk usage",
            "storage left",
            "storage space",
            "out of space",
            "low on space",
            "low on storage",
            "storage full",
            "storage is full",
            "drive is full",
            "disk is full",
        ],
    ) || (contains_word(&text, "how much space")
        && (about_own_stuff(&text) || has_any(&text, &["left", "free"]) || words <= 4));
    if act && wants_empty_safe_place(&text) {
        return Intent::EmptySafePlace;
    }
    if act
        && has_any(
            &text,
            &[
                "temp files",
                "temp file",
                "temporary files",
                "temp folder",
                "temporary folder",
                "%temp%",
                "temp data",
            ],
        )
        && has_any(
            &text,
            &[
                "clean", "clear", "delete", "remove", "empty", "free", "get rid",
            ],
        )
    {
        return Intent::CleanTempFiles;
    }
    if wants_suggestions(&text) {
        return Intent::Suggestions;
    }
    if has_any(
        &text,
        &[
            "is my data safe",
            "is my data secure",
            "are my files safe",
            "is my information safe",
            "are you safe",
            "are you secure",
            "how secure are you",
            "how safe are you",
            "keep my data safe",
            "protect my data",
            "protect my files",
            "protect my privacy",
            "security status",
            "privacy status",
            "do you upload",
            "do you send my",
            "can anyone see my",
            "who can see my",
            "where is my data",
            "where do you keep my data",
            "is it private",
        ],
    ) {
        return Intent::SecurityStatus;
    }
    if wants_open_apps(&text) {
        return Intent::ListOpenApps;
    }
    if let Some(intent) = close_target(&text) {
        return intent;
    }
    if act && disk_phrase {
        return Intent::DiskSpace;
    }
    if act
        && has_any(
            &text,
            &["undo", "bring back", "restore", "put back", "revert"],
        )
    {
        return Intent::Undo;
    }
    if act
        && (has_any(
            &text,
            &[
                "what did you do",
                "what have you done",
                "recent changes",
                "show changes",
                "cleanup history",
                "clean-up history",
                "your history",
                "show history",
            ],
        ) || (contains_word(&text, "history") && words <= 2))
    {
        return Intent::History;
    }
    if let Some((name, scope)) = find_target(&text) {
        // "where is my downloads folder?" means the folder itself.
        let folder_word = name.strip_suffix(" folder").unwrap_or(&name);
        if let (Some(folder), Scope::UserFolders) = (KnownFolder::from_word(folder_word), &scope) {
            return Intent::OpenFolder(folder);
        }
        return Intent::FindFiles { name, scope };
    }
    if act && about_system(&text) {
        return Intent::SystemStatus;
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
    if words <= 5
        && !is_question(&text)
        && has_any(
            &text,
            &["go to sleep", "take a nap", "sleep now", "nap time", "nap"],
        )
    {
        return Intent::Sleep;
    }
    if let Some((target, strict)) = strip_open(&text) {
        if !target.is_empty() {
            match open_target(target) {
                Intent::OpenUnknown(_) if !strict || word_count(target) > 3 => {}
                intent => return intent,
            }
        }
    }
    if !contains_word(&text, "in")
        && has_any(
            &text,
            &[
                "what time",
                "time is it",
                "current time",
                "time now",
                "tell me the time",
                "whats the time",
                "what's the time",
            ],
        )
    {
        return Intent::Time;
    }
    if has_any(
        &text,
        &[
            "what date",
            "whats the date",
            "what's the date",
            "what is the date",
            "today's date",
            "todays date",
            "what day is it",
            "what day is today",
            "which day is today",
            "date today",
        ],
    ) {
        return Intent::Date;
    }
    if (contains_word(&text, "help") && words <= 3)
        || has_any(
            &text,
            &[
                "what can you do",
                "what do you do",
                "list commands",
                "show commands",
                "your commands",
            ],
        )
    {
        return Intent::Help;
    }
    if words <= 5 && has_any(&text, &["who are you", "your name", "what are you"]) {
        return Intent::WhoAreYou;
    }
    if words <= 8 && has_any(&text, &["thanks", "thank you", "thx", "ty"]) {
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
    if words <= 4 && has_any(&text, &greeting) {
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
    fn file_names_are_never_websites() {
        assert_eq!(
            parse("open resume.pdf"),
            Intent::OpenUnknown("resume.pdf".into())
        );
        assert_eq!(
            parse("open invoice.zip"),
            Intent::OpenUnknown("invoice.zip".into())
        );
        assert_eq!(
            parse("open holiday.mov"),
            Intent::OpenUnknown("holiday.mov".into())
        );
        assert_eq!(url_for_domain("setup.exe"), None);
        assert_eq!(
            parse("open example.com"),
            Intent::OpenWebsite {
                name: "example.com".into(),
                url: "https://example.com".into()
            }
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

    #[test]
    fn questions_about_the_world_never_start_a_scan_or_undo() {
        for question in [
            "what is the largest planet?",
            "explain what duplicate files are",
            "how do I restore a backup?",
            "how much space does a 4k movie take?",
            "I don't have the time",
            "can you help me write an email to my boss",
            "show me how to make pasta",
            "what's the history of rome?",
            "remove duplicates from this list: 1, 2, 2, 3",
            "best cloud storage",
            "how long should a nap be?",
            "how does a cpu work?",
            "start a timer for 5 minutes",
        ] {
            assert_eq!(parse(question), Intent::Unknown, "{question}");
        }
    }

    #[test]
    fn questions_about_the_persons_own_files_still_act() {
        assert_eq!(
            parse("what are the largest files?"),
            Intent::LargestFiles(Scope::UserFolders)
        );
        assert_eq!(
            parse("What are the biggest files in my Downloads?"),
            Intent::LargestFiles(Scope::Folders(vec![KnownFolder::Downloads]))
        );
        assert_eq!(
            parse("how do I find duplicates in my pictures"),
            Intent::FindDuplicates(Scope::Folders(vec![KnownFolder::Pictures]))
        );
        assert_eq!(parse("am I running out of space?"), Intent::DiskSpace);
        assert_eq!(parse("open the github website"), parse("open github"));
        assert_eq!(
            parse("open frobnicator"),
            Intent::OpenUnknown("frobnicator".into())
        );
    }

    #[test]
    fn finds_files_by_name() {
        assert_eq!(
            parse("where is my resume?"),
            Intent::FindFiles {
                name: "resume".into(),
                scope: Scope::UserFolders
            }
        );
        assert_eq!(
            parse("find my tax documents"),
            Intent::FindFiles {
                name: "tax".into(),
                scope: Scope::UserFolders
            }
        );
        assert_eq!(
            parse("Where's my invoice 2025 in Documents?"),
            Intent::FindFiles {
                name: "invoice 2025".into(),
                scope: Scope::Folders(vec![KnownFolder::Documents])
            }
        );
        assert_eq!(
            parse("search my pc for holiday photos"),
            Intent::FindFiles {
                name: "holiday photos".into(),
                scope: Scope::UserFolders
            }
        );
        assert_eq!(
            parse("where is my downloads folder"),
            Intent::OpenFolder(KnownFolder::Downloads)
        );
        // Still the dedicated tools, and web searches stay web searches.
        assert_eq!(
            parse("find my duplicates"),
            Intent::FindDuplicates(Scope::UserFolders)
        );
        assert_eq!(
            parse("search for cheap flights"),
            Intent::WebSearch("cheap flights".into())
        );
    }

    #[test]
    fn reports_on_the_system() {
        for message in [
            "battery",
            "how's my battery?",
            "is my pc slow?",
            "why is my laptop so slow",
            "what's my cpu usage",
            "how much ram do I have",
        ] {
            assert_eq!(parse(message), Intent::SystemStatus, "{message}");
        }
    }

    #[test]
    fn empties_the_safe_place_only_when_asked_to() {
        for message in [
            "empty the safe place",
            "Empty your safe place please",
            "clear the quarantine",
            "free the space now",
            "free up the space now in the safe place",
        ] {
            assert_eq!(parse(message), Intent::EmptySafePlace, "{message}");
        }
        for message in [
            "what's in your safe place?",
            "how much is in the safe place",
            "empty the recycle bin",
            "undo",
        ] {
            assert_ne!(parse(message), Intent::EmptySafePlace, "{message}");
        }
    }

    #[test]
    fn closes_apps_by_name_and_lists_them() {
        assert_eq!(parse("close notepad"), Intent::CloseApp("notepad".into()));
        assert_eq!(
            parse("Quit Chrome please"),
            Intent::CloseApp("chrome".into())
        );
        assert_eq!(
            parse("exit the calculator"),
            Intent::CloseApp("the calculator".into())
        );
        assert_eq!(parse("kill spotify"), Intent::CloseApp("spotify".into()));
        assert_eq!(parse("close yourself"), Intent::CloseCubby);
        assert_eq!(parse("close the chat"), Intent::CloseCubby);
        for message in [
            "what apps are open?",
            "what's open",
            "What is running on my PC?",
            "which apps are running right now",
            "open apps",
        ] {
            assert_eq!(parse(message), Intent::ListOpenApps, "{message}");
        }
    }

    #[test]
    fn close_and_open_words_in_other_sentences_go_to_the_model() {
        for message in [
            "what's open near me?",
            "close the deal with my boss next week please",
            "how do I close a bank account?",
            "is the shop open today?",
        ] {
            assert!(
                !matches!(parse(message), Intent::CloseApp(_) | Intent::ListOpenApps),
                "{message}"
            );
        }
    }

    #[test]
    fn understands_reminders_and_the_to_do_list() {
        assert_eq!(
            parse("Remind me to call Mom at 5pm"),
            Intent::AddTask("call Mom at 5pm".into())
        );
        assert_eq!(
            parse("remind me in 20 minutes to check the oven."),
            Intent::AddTask("in 20 minutes to check the oven".into())
        );
        assert_eq!(
            parse("set a reminder for tomorrow at 9am to pay rent"),
            Intent::AddTask("tomorrow at 9am to pay rent".into())
        );
        assert_eq!(
            parse("Add Oat Milk to my list"),
            Intent::AddTask("Oat Milk".into())
        );
        assert_eq!(
            parse("add eggs to my to-do list"),
            Intent::AddTask("eggs".into())
        );
        assert_eq!(
            parse("new task: renew passport"),
            Intent::AddTask("renew passport".into())
        );
        for message in [
            "what's on my list?",
            "show my reminders",
            "my to-do list",
            "what do I have to do?",
            "tasks",
            "open my list",
            "open my to-do list",
        ] {
            assert_eq!(parse(message), Intent::ListTasks, "{message}");
        }
        assert_eq!(
            parse("mark milk as done"),
            Intent::CompleteTask("milk".into())
        );
        assert_eq!(parse("mark 2 done"), Intent::CompleteTask("2".into()));
        assert_eq!(
            parse("done with the task taxes"),
            Intent::CompleteTask("taxes".into())
        );
        assert_eq!(
            parse("tick off buy eggs"),
            Intent::CompleteTask("buy eggs".into())
        );
        assert_eq!(
            parse("remove eggs from my list"),
            Intent::RemoveTask("eggs".into())
        );
        assert_eq!(
            parse("cancel the reminder to call mom"),
            Intent::RemoveTask("call mom".into())
        );
    }

    #[test]
    fn task_words_never_trigger_other_tools() {
        assert_eq!(
            parse("remind me to put back the chairs"),
            Intent::AddTask("put back the chairs".into())
        );
        assert_eq!(
            parse("remind me to find duplicates tomorrow"),
            Intent::AddTask("find duplicates tomorrow".into())
        );
        assert_eq!(parse("undo"), Intent::Undo);
    }

    #[test]
    fn explains_security_when_asked() {
        for message in [
            "is my data safe?",
            "How do you protect my privacy?",
            "do you upload my files anywhere?",
            "security status",
            "who can see my files?",
        ] {
            assert_eq!(parse(message), Intent::SecurityStatus, "{message}");
        }
    }

    #[test]
    fn gives_suggestions_when_asked() {
        for message in [
            "any suggestions?",
            "health check",
            "Check my PC",
            "how can I free up space?",
            "please help me free up space",
            "speed up my laptop",
            "what should I clean?",
        ] {
            assert_eq!(parse(message), Intent::Suggestions, "{message}");
        }
        assert_eq!(parse("how much free space?"), Intent::DiskSpace);
        assert_eq!(parse("empty the safe place"), Intent::EmptySafePlace);
        assert_eq!(parse("free the space now"), Intent::EmptySafePlace);
    }

    #[test]
    fn cleans_temp_files_when_asked() {
        for message in [
            "clean temp files",
            "Delete my temporary files",
            "clear the temp folder please",
            "free up space from temp files",
        ] {
            assert_eq!(parse(message), Intent::CleanTempFiles, "{message}");
        }
        assert_ne!(parse("what are temp files?"), Intent::CleanTempFiles);
    }

    #[test]
    fn looks_up_tool_names() {
        assert_eq!(
            KnownFolder::from_label("pictures"),
            Some(KnownFolder::Pictures)
        );
        assert_eq!(KnownFolder::from_label("C:\\Windows"), None);
        assert_eq!(
            App::from_tool_name("file_explorer"),
            Some(App::FileExplorer)
        );
        assert_eq!(App::from_tool_name("cmd"), None);
        assert_eq!(
            website("YouTube"),
            Some(("youtube".into(), "https://www.youtube.com".into()))
        );
        assert_eq!(
            website("example.org"),
            Some(("example.org".into(), "https://example.org".into()))
        );
        assert_eq!(website("not a site"), None);
    }
}
