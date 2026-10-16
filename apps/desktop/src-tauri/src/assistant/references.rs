//! Resolve follow-ups against files actually shown, never a guessed basename.
use custodian_core::walk::FileEntry;
use custodian_winsec::fsops::{file_identity, FileIdentity};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

struct Observation {
    path: PathBuf,
    identity: FileIdentity,
    bytes: u64,
    modified: Option<SystemTime>,
}

#[derive(Default)]
struct ReferenceState {
    generation: u64,
    files: Vec<Option<Observation>>,
}

#[derive(Default)]
pub(super) struct FileReferences(Mutex<ReferenceState>);

impl FileReferences {
    pub fn begin_search(&self) -> u64 {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.generation = state.generation.wrapping_add(1);
        state.files.clear();
        state.generation
    }

    pub fn complete_search(&self, generation: u64, files: &[FileEntry]) {
        let observations = files
            .iter()
            .map(|file| {
                custodian_core::privacy::check(&file.path).ok()?;
                let path = file.path.canonicalize().ok()?;
                let meta = std::fs::metadata(&path).ok()?;
                if meta.len() != file.size || meta.modified().ok() != file.modified {
                    return None;
                }
                Some(Observation {
                    identity: file_identity(&path).ok()?,
                    path,
                    bytes: meta.len(),
                    modified: meta.modified().ok(),
                })
            })
            .collect();
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.generation == generation {
            state.files = observations;
        }
    }

    pub fn clear(&self) {
        self.begin_search();
    }

    #[cfg(test)]
    pub fn replace(&self, files: &[FileEntry]) {
        self.complete_search(self.begin_search(), files);
    }

    /// None means a literal filename, not a conversational reference.
    pub fn resolve(&self, text: &str) -> Option<Result<PathBuf, String>> {
        let text = text.trim().to_ascii_lowercase();
        let index = match text.as_str() {
            "it" | "that" | "that file" | "this file" | "the file" => None,
            "first" | "first file" | "the first file" | "the first one" | "file 1" => Some(0),
            "second" | "second file" | "the second file" | "the second one" | "file 2" => Some(1),
            "third" | "third file" | "the third file" | "the third one" | "file 3" => Some(2),
            _ => return None,
        };
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let files = &state.files;
        let index = match index {
            Some(index) => index,
            None if files.len() == 1 => 0,
            None => return Some(Err(
                "Which file do you mean? Give its full path or its position in the latest results."
                    .into(),
            )),
        };
        Some((|| {
            let file = files
                .get(index)
                .and_then(Option::as_ref)
                .ok_or("That result is no longer available. Search again or give the full path.")?;
            custodian_core::privacy::check(&file.path)
                .map_err(|_| custodian_core::privacy::DENIED)?;
            let meta = std::fs::metadata(&file.path)
                .map_err(|_| "That file is no longer available. Search again.")?;
            if file_identity(&file.path).ok() != Some(file.identity)
                || meta.len() != file.bytes
                || meta.modified().ok() != file.modified
            {
                return Err(
                    "That file changed since I showed it. Search again before changing it.".into(),
                );
            }
            Ok(file.path.clone())
        })())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use custodian_core::{largest::largest_files, walk::WalkLimits};
    use std::sync::atomic::AtomicBool;

    #[test]
    fn followup_selects_measured_object_and_rejects_replacement_or_ambiguity() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(&a, b"small").unwrap();
        std::fs::write(&b, b"largest payload").unwrap();
        let (files, _) = largest_files(
            &[dir.path().into()],
            2,
            WalkLimits::default(),
            &AtomicBool::new(false),
            &mut |_| {},
        );
        let refs = FileReferences::default();
        refs.replace(&files);
        assert!(refs.resolve("it").unwrap().is_err());
        assert_eq!(
            refs.resolve("the first file").unwrap().unwrap(),
            b.canonicalize().unwrap()
        );
        refs.replace(&files[..1]);
        assert_eq!(
            refs.resolve("it").unwrap().unwrap(),
            b.canonicalize().unwrap()
        );
        std::fs::rename(&b, dir.path().join("old-b.txt")).unwrap();
        std::fs::write(&b, b"largest payload").unwrap();
        assert!(refs.resolve("it").unwrap().is_err());
        refs.clear();
        assert!(refs.resolve("it").unwrap().is_err());
        assert!(refs.resolve("literal.txt").is_none());
        let stale = refs.begin_search();
        refs.clear();
        refs.complete_search(stale, &files[1..]);
        assert!(
            refs.resolve("it").unwrap().is_err(),
            "old scans cannot repopulate a fresh conversation"
        );
    }
}
