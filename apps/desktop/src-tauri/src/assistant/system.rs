//! "Is my PC slow?": processor and memory use, the apps using the most,
//! battery and uptime. Read-only; nothing here changes the PC.

use custodian_winsec::power::{self, Battery};
use std::collections::HashMap;
use std::time::Duration;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, MINIMUM_CPU_UPDATE_INTERVAL};

/// One app's share of the PC, all of its processes together.
#[derive(Debug, Clone, PartialEq)]
pub struct AppUsage {
    pub name: String,
    pub memory_bytes: u64,
    /// Share of the whole processor, 0–100.
    pub cpu_percent: f32,
}

/// The state of the PC at one moment.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// Share of the whole processor in use, 0–100.
    pub cpu_percent: f32,
    pub memory_used: u64,
    pub memory_total: u64,
    /// Apps by memory use, biggest first.
    pub apps: Vec<AppUsage>,
    pub battery: Option<Battery>,
    pub uptime: Duration,
}

impl Snapshot {
    /// Memory in use, 0–100.
    pub fn memory_percent(&self) -> f64 {
        if self.memory_total == 0 {
            0.0
        } else {
            self.memory_used as f64 / self.memory_total as f64 * 100.0
        }
    }

    /// The app using the most processor time.
    pub fn busiest_app(&self) -> Option<&AppUsage> {
        self.apps
            .iter()
            .max_by(|a, b| a.cpu_percent.total_cmp(&b.cpu_percent))
    }
}

/// Parts of Windows itself that mean nothing to a person.
const HIDDEN: &[&str] = &[
    "system",
    "system idle process",
    "idle",
    "registry",
    "memory compression",
    "memcompression",
    "secure system",
];

/// Names people know for common programs, by executable name.
const FRIENDLY: &[(&str, &str)] = &[
    ("chrome", "Chrome"),
    ("msedge", "Microsoft Edge"),
    ("firefox", "Firefox"),
    ("brave", "Brave"),
    ("opera", "Opera"),
    ("code", "VS Code"),
    ("ms-teams", "Teams"),
    ("teams", "Teams"),
    ("explorer", "File Explorer"),
    ("msedgewebview2", "Edge WebView (used by apps)"),
    ("discord", "Discord"),
    ("spotify", "Spotify"),
    ("whatsapp", "WhatsApp"),
    ("winword", "Word"),
    ("excel", "Excel"),
    ("powerpnt", "PowerPoint"),
    ("outlook", "Outlook"),
    ("olk", "Outlook"),
    ("onedrive", "OneDrive"),
    ("msmpeng", "Microsoft Defender"),
    ("searchhost", "Windows Search"),
    ("svchost", "Windows services"),
    ("dwm", "Desktop Window Manager"),
    ("vmmem", "Virtual machines (WSL)"),
    ("vmmemwsl", "Virtual machines (WSL)"),
    ("node", "Node.js"),
    ("python", "Python"),
    ("java", "Java"),
    ("javaw", "Java"),
    ("steam", "Steam"),
    ("zoom", "Zoom"),
    ("slack", "Slack"),
    ("notepad", "Notepad"),
    ("mspaint", "Paint"),
    ("taskmgr", "Task Manager"),
    ("custodian-desktop", "Cubby"),
];

/// "chrome.exe" → "Chrome"; `None` for parts of Windows people never see.
pub fn friendly_name(process: &str) -> Option<String> {
    let stem = match process.get(process.len().saturating_sub(4)..) {
        Some(ext) if ext.eq_ignore_ascii_case(".exe") => &process[..process.len() - 4],
        _ => process,
    };
    let lower = stem.trim().to_lowercase();
    if lower.is_empty() || HIDDEN.contains(&lower.as_str()) {
        return None;
    }
    if let Some((_, name)) = FRIENDLY.iter().find(|(exe, _)| *exe == lower) {
        return Some((*name).to_string());
    }
    let mut chars = stem.trim().chars();
    chars
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
}

/// Measures the PC. Takes a fraction of a second, because processor use is
/// the difference between two readings.
pub fn snapshot() -> Snapshot {
    let mut sys = System::new();
    let processes = ProcessRefreshKind::nothing().with_memory().with_cpu();
    sys.refresh_cpu_usage();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, processes);
    std::thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL.max(Duration::from_millis(300)));
    sys.refresh_cpu_usage();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, processes);
    sys.refresh_memory();

    // A process's processor use is relative to one core.
    let cores = match sys.cpus().len() {
        0 => std::thread::available_parallelism().map_or(1, |n| n.get()),
        n => n,
    } as f32;
    let mut by_app: HashMap<String, AppUsage> = HashMap::new();
    for process in sys.processes().values() {
        let Some(name) = friendly_name(&process.name().to_string_lossy()) else {
            continue;
        };
        let usage = by_app.entry(name.clone()).or_insert(AppUsage {
            name,
            memory_bytes: 0,
            cpu_percent: 0.0,
        });
        usage.memory_bytes += process.memory();
        usage.cpu_percent += process.cpu_usage() / cores;
    }
    let mut apps: Vec<AppUsage> = by_app.into_values().collect();
    apps.sort_by(|a, b| {
        b.memory_bytes
            .cmp(&a.memory_bytes)
            .then_with(|| a.name.cmp(&b.name))
    });

    Snapshot {
        cpu_percent: sys.global_cpu_usage().clamp(0.0, 100.0),
        memory_used: sys.used_memory(),
        memory_total: sys.total_memory(),
        apps,
        battery: power::battery(),
        uptime: Duration::from_secs(System::uptime()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_programs_the_way_people_know_them() {
        assert_eq!(friendly_name("chrome.exe").as_deref(), Some("Chrome"));
        assert_eq!(
            friendly_name("MSEDGE.EXE").as_deref(),
            Some("Microsoft Edge")
        );
        assert_eq!(friendly_name("Code.exe").as_deref(), Some("VS Code"));
        assert_eq!(friendly_name("blender.exe").as_deref(), Some("Blender"));
        assert_eq!(friendly_name("Memory Compression"), None);
        assert_eq!(friendly_name("System"), None);
        assert_eq!(friendly_name(".exe"), None);
        assert_eq!(friendly_name(""), None);
        // Multi-byte names never split inside a character.
        assert_eq!(friendly_name("éditeur").as_deref(), Some("Éditeur"));
        assert_eq!(friendly_name("日本語.exe").as_deref(), Some("日本語"));
    }

    #[test]
    fn measures_the_real_pc_without_panicking() {
        let s = snapshot();
        assert!((0.0..=100.0).contains(&s.cpu_percent));
        assert!(s.memory_total >= s.memory_used);
        assert!(s
            .apps
            .windows(2)
            .all(|w| w[0].memory_bytes >= w[1].memory_bytes));
        assert!((0.0..=100.0).contains(&s.memory_percent()));
    }
}
