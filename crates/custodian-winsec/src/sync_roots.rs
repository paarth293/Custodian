//! Folders that cloud storage keeps in sync: OneDrive (including Desktop,
//! Documents and Pictures when Windows has moved them there), Dropbox,
//! iCloud and anything else registered with Windows as a sync root.
//!
//! A change inside one of these also happens in the cloud and on the
//! person's other devices, so Custodian treats them as read-only zones: it
//! may look and explain, but it never proposes moving or removing anything
//! there (K-14, §35).

use std::path::{Component, Path, PathBuf};

/// A synced folder and the service that syncs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncRoot {
    pub path: PathBuf,
    /// "OneDrive", "Dropbox", "iCloud", … for messages to the person.
    pub provider: String,
}

/// Every synced folder Custodian can find for the current user.
pub fn sync_roots() -> Vec<SyncRoot> {
    let mut found = Vec::new();
    let mut add = |path: PathBuf, provider: &str| {
        found.push(SyncRoot {
            path,
            provider: provider.to_string(),
        })
    };
    for var in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
        if let Some(value) = std::env::var_os(var) {
            add(PathBuf::from(value), "OneDrive");
        }
    }
    #[cfg(windows)]
    {
        use crate::registry::{string_value, string_values, subkeys, Hive};

        const ACCOUNTS: &str = r"Software\Microsoft\OneDrive\Accounts";
        for account in subkeys(Hive::CurrentUser, ACCOUNTS) {
            let key = format!(r"{ACCOUNTS}\{account}");
            if let Some(folder) = string_value(Hive::CurrentUser, &key, "UserFolder") {
                add(PathBuf::from(folder), "OneDrive");
            }
        }
        // OneDrive and SharePoint libraries mounted by the sync client.
        const PROVIDERS: &str = r"Software\SyncEngines\Providers";
        for provider in subkeys(Hive::CurrentUser, PROVIDERS) {
            let base = format!(r"{PROVIDERS}\{provider}");
            for mount in subkeys(Hive::CurrentUser, &base) {
                let key = format!(r"{base}\{mount}");
                if let Some(point) = string_value(Hive::CurrentUser, &key, "MountPoint") {
                    add(PathBuf::from(point), &provider);
                }
            }
        }
        // Every provider that uses the Windows cloud files API.
        const MANAGER: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\SyncRootManager";
        for root in subkeys(Hive::LocalMachine, MANAGER) {
            let provider = root.split('!').next().unwrap_or(&root).to_string();
            let key = format!(r"{MANAGER}\{root}\UserSyncRoots");
            for (_user, path) in string_values(Hive::LocalMachine, &key) {
                add(PathBuf::from(path), &provider);
            }
        }
    }
    for var in ["APPDATA", "LOCALAPPDATA"] {
        if let Some(dir) = std::env::var_os(var) {
            let info = Path::new(&dir).join("Dropbox").join("info.json");
            if let Ok(text) = std::fs::read_to_string(info) {
                for path in dropbox_folders(&text) {
                    add(path, "Dropbox");
                }
            }
        }
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        let icloud = Path::new(&profile).join("iCloudDrive");
        if icloud.is_dir() {
            add(icloud, "iCloud");
        }
    }
    tidy(found)
}

/// The folders listed in Dropbox's `info.json` ({"personal": {"path": …}}).
fn dropbox_folders(json: &str) -> Vec<PathBuf> {
    let Ok(serde_json::Value::Object(accounts)) = serde_json::from_str(json) else {
        return Vec::new();
    };
    accounts
        .values()
        .filter_map(|account| account.get("path")?.as_str())
        .map(PathBuf::from)
        .collect()
}

/// Drops relative or empty paths and duplicates (compared without case).
fn tidy(roots: Vec<SyncRoot>) -> Vec<SyncRoot> {
    let mut kept: Vec<SyncRoot> = Vec::new();
    for root in roots {
        if !root.path.is_absolute() {
            continue;
        }
        if kept.iter().any(|k| same_path(&k.path, &root.path)) {
            continue;
        }
        kept.push(root);
    }
    kept
}

/// Path components compared the way Windows does: without case, and
/// ignoring a trailing separator.
fn parts(path: &Path) -> Vec<String> {
    path.components()
        .filter(|c| !matches!(c, Component::CurDir))
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect()
}

fn same_path(a: &Path, b: &Path) -> bool {
    parts(a) == parts(b)
}

/// The synced folder `path` is inside, if any.
pub fn sync_root_of<'a>(path: &Path, roots: &'a [SyncRoot]) -> Option<&'a SyncRoot> {
    let path = parts(path);
    roots.iter().find(|root| {
        let root = parts(&root.path);
        !root.is_empty() && path.len() >= root.len() && path[..root.len()] == root[..]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(path: &str, provider: &str) -> SyncRoot {
        SyncRoot {
            path: PathBuf::from(path),
            provider: provider.into(),
        }
    }

    #[cfg(windows)]
    #[test]
    fn finds_paths_inside_a_synced_folder_without_case() {
        let roots = vec![root(r"C:\Users\me\OneDrive", "OneDrive")];
        let inside = |p: &str| sync_root_of(Path::new(p), &roots).map(|r| r.provider.as_str());
        assert_eq!(
            inside(r"C:\Users\me\OneDrive\Desktop\a.jpg"),
            Some("OneDrive")
        );
        assert_eq!(inside(r"c:\users\ME\onedrive\Pictures"), Some("OneDrive"));
        assert_eq!(inside(r"C:\Users\me\OneDrive"), Some("OneDrive"));
        assert_eq!(
            inside(r"C:\Users\me\OneDrive Backup\a.jpg"),
            None,
            "a different folder"
        );
        assert_eq!(inside(r"C:\Users\me\Downloads\a.jpg"), None);
    }

    #[test]
    fn reads_dropbox_folders() {
        let json = r#"{"personal": {"path": "C:\\Users\\me\\Dropbox", "host": 1},
                       "business": {"path": "C:\\Users\\me\\Dropbox (Work)"}}"#;
        let mut found = dropbox_folders(json);
        found.sort();
        assert_eq!(
            found,
            vec![
                PathBuf::from(r"C:\Users\me\Dropbox"),
                PathBuf::from(r"C:\Users\me\Dropbox (Work)")
            ]
        );
        assert!(dropbox_folders("not json").is_empty());
        assert!(dropbox_folders("[1, 2]").is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn tidies_duplicates_and_relative_paths() {
        let roots = tidy(vec![
            root(r"C:\Users\me\OneDrive", "OneDrive"),
            root(r"c:\users\me\onedrive\", "OneDrive"),
            root("relative", "Odd"),
            root("", "Empty"),
        ]);
        assert_eq!(roots, vec![root(r"C:\Users\me\OneDrive", "OneDrive")]);
    }

    #[test]
    fn finding_the_real_sync_roots_never_panics() {
        for root in sync_roots() {
            assert!(root.path.is_absolute());
            assert!(!root.provider.is_empty());
        }
    }
}
