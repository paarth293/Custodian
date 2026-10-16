//! Temporary files apps leave behind (§27, rung 2 "Regenerate"). They are
//! deleted rather than moved to the safe place, because freeing their space
//! is the point and apps recreate what they need (R1: the function comes
//! back, the bytes do not). Only regular files in the person's own temp
//! folder, untouched for a week, are ever proposed, and each is checked
//! again just before it is deleted.

use crate::walk::{walk_files, WalkLimits};
use custodian_approval::{Approval, ApprovalError, PlanHash, Verifier};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Files younger than this may still be in use by an install or an app.
pub const MIN_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Most files proposed at once.
const MAX_ITEMS: usize = 50_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TempItem {
    pub path: PathBuf,
    pub size: u64,
    /// Last change, so a file used again after the scan is left alone.
    pub modified_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TempPlan {
    pub id: String,
    /// The temp folder every item must be inside.
    pub root: PathBuf,
    pub items: Vec<TempItem>,
}

/// Result of deleting a plan's files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TempOutcome {
    pub deleted: usize,
    pub bytes: u64,
    /// Files left alone: in use, changed since the scan, or no longer safe.
    pub skipped: usize,
}

fn unix(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl TempPlan {
    pub fn total_bytes(&self) -> u64 {
        self.items.iter().map(|i| i.size).sum()
    }

    /// Canonical digest of everything the plan will delete; approvals sign it.
    pub fn digest(&self) -> PlanHash {
        fn field(hasher: &mut blake3::Hasher, bytes: &[u8]) {
            hasher.update(&(bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        }
        let mut hasher = blake3::Hasher::new();
        field(&mut hasher, b"custodian-temp-plan-v1");
        field(&mut hasher, self.id.as_bytes());
        field(&mut hasher, self.root.as_os_str().as_encoded_bytes());
        for item in &self.items {
            field(&mut hasher, item.path.as_os_str().as_encoded_bytes());
            field(&mut hasher, &item.size.to_le_bytes());
            field(&mut hasher, &item.modified_unix.to_le_bytes());
        }
        hasher.finalize().into()
    }
}

/// The person's own temp folder, if it is where Windows normally puts it:
/// a folder called Temp inside their local app data. Anything else (a
/// system folder, a whole drive) is never cleaned.
pub fn user_temp_dir() -> Option<PathBuf> {
    // Canonical forms, so short (8.3) names and links compare correctly.
    let temp = fs::canonicalize(std::env::temp_dir()).ok()?;
    let local = fs::canonicalize(std::env::var_os("LOCALAPPDATA")?).ok()?;
    let named_temp = temp
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("temp"));
    let inside = temp
        .parent()
        .is_some_and(|p| p.as_os_str().eq_ignore_ascii_case(local.as_os_str()));
    (named_temp && inside && temp.is_dir()).then_some(temp)
}

/// Finds files under `root` untouched since before `now - MIN_AGE`.
pub fn scan(root: &Path, now: SystemTime, cancel: &AtomicBool) -> TempPlan {
    let cutoff = unix(now).saturating_sub(MIN_AGE.as_secs());
    let mut items = Vec::new();
    walk_files(
        &[root.to_path_buf()],
        WalkLimits {
            max_files: 200_000,
            min_size: 1,
            ..WalkLimits::default()
        },
        cancel,
        &mut |_| {},
        &mut |entry| {
            let Some(modified) = entry.modified.map(unix) else {
                return;
            };
            if modified <= cutoff && items.len() < MAX_ITEMS {
                items.push(TempItem {
                    path: entry.path,
                    size: entry.size,
                    modified_unix: modified,
                });
            }
        },
    );
    items.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
    TempPlan {
        id: uuid::Uuid::new_v4().simple().to_string()[..12].to_string(),
        root: root.to_path_buf(),
        items,
    }
}

/// Whether `item` may still be deleted: inside the temp folder (links
/// resolved), a plain file, and unchanged since the scan.
fn still_safe(item: &TempItem, root: &Path) -> bool {
    if custodian_winsec::SafePath::new(&item.path, root).is_err() {
        return false;
    }
    let Ok(meta) = fs::symlink_metadata(&item.path) else {
        return false;
    };
    meta.file_type().is_file()
        && !is_reparse_point(&meta)
        && meta.len() == item.size
        && meta.modified().map(unix).ok() == Some(item.modified_unix)
}

#[cfg(windows)]
fn is_reparse_point(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_meta: &fs::Metadata) -> bool {
    false
}

/// Deletes the plan's files, only if `approval` signs exactly this plan
/// (SR-D1). Files in use or changed since the scan are skipped.
pub fn delete_approved(
    plan: &TempPlan,
    approval: &Approval,
    verifier: &Verifier,
    now_unix: u64,
) -> Result<TempOutcome, ApprovalError> {
    verifier.verify(&plan.digest(), approval, now_unix)?;
    let mut outcome = TempOutcome::default();
    for item in &plan.items {
        if still_safe(item, &plan.root) && fs::remove_file(&item.path).is_ok() {
            outcome.deleted += 1;
            outcome.bytes += item.size;
        } else {
            outcome.skipped += 1;
        }
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use custodian_approval::ApprovalKey;

    fn aged(path: &Path, body: &[u8], days: u64) {
        fs::write(path, body).unwrap();
        let when = SystemTime::now() - Duration::from_secs(days * 24 * 60 * 60);
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(when)
            .unwrap();
    }

    fn approve(plan: &TempPlan) -> (Approval, Verifier, u64) {
        let key = ApprovalKey::generate();
        let now = unix(SystemTime::now());
        (
            key.sign(&plan.digest(), now, 120),
            Verifier::new(key.verifying_key()),
            now,
        )
    }

    #[test]
    fn proposes_only_files_older_than_a_week() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("setup123")).unwrap();
        aged(&dir.path().join("old.log"), b"old log", 30);
        aged(&dir.path().join("setup123/cache.bin"), b"leftover", 10);
        aged(&dir.path().join("fresh.tmp"), b"in use", 1);
        let plan = scan(dir.path(), SystemTime::now(), &AtomicBool::new(false));
        let mut names: Vec<String> = plan
            .items
            .iter()
            .map(|i| i.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, vec!["cache.bin", "old.log"]);
        assert_eq!(plan.total_bytes(), 15);
    }

    #[test]
    fn deletes_approved_files_and_skips_changed_ones() {
        let dir = tempfile::tempdir().unwrap();
        aged(&dir.path().join("a.tmp"), b"aaaa", 20);
        aged(&dir.path().join("b.tmp"), b"bbbb", 20);
        let plan = scan(dir.path(), SystemTime::now(), &AtomicBool::new(false));
        assert_eq!(plan.items.len(), 2);
        // An app uses b.tmp again after the scan.
        fs::write(dir.path().join("b.tmp"), b"busy again").unwrap();
        let (approval, verifier, now) = approve(&plan);
        let outcome = delete_approved(&plan, &approval, &verifier, now).unwrap();
        assert_eq!(outcome.deleted, 1);
        assert_eq!(outcome.skipped, 1);
        assert!(!dir.path().join("a.tmp").exists());
        assert!(dir.path().join("b.tmp").exists());
    }

    #[test]
    fn deletes_nothing_without_the_right_approval() {
        let dir = tempfile::tempdir().unwrap();
        aged(&dir.path().join("a.tmp"), b"aaaa", 20);
        let plan = scan(dir.path(), SystemTime::now(), &AtomicBool::new(false));
        let (approval, verifier, now) = approve(&plan);
        let mut other = plan.clone();
        other.items[0].path = dir.path().join("elsewhere.tmp");
        assert!(delete_approved(&other, &approval, &verifier, now).is_err());
        assert!(dir.path().join("a.tmp").exists());
    }

    #[test]
    fn never_deletes_outside_the_temp_folder() {
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        aged(&outside.path().join("precious.docx"), b"thesis", 400);
        let plan = TempPlan {
            id: "p".into(),
            root: temp.path().to_path_buf(),
            items: vec![TempItem {
                path: outside.path().join("precious.docx"),
                size: 6,
                modified_unix: unix(
                    fs::metadata(outside.path().join("precious.docx"))
                        .unwrap()
                        .modified()
                        .unwrap(),
                ),
            }],
        };
        let (approval, verifier, now) = approve(&plan);
        let outcome = delete_approved(&plan, &approval, &verifier, now).unwrap();
        assert_eq!(outcome.deleted, 0);
        assert!(outside.path().join("precious.docx").exists());
    }

    #[test]
    fn only_cleans_a_temp_folder_inside_local_app_data() {
        if let Some(temp) = user_temp_dir() {
            assert!(temp.ends_with("Temp") || temp.ends_with("temp"));
        }
    }
}
