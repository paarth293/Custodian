//! Privacy and identity checks for future application adapters.
//!
//! This module intentionally does not inspect window content or grant UI
//! authority. An adapter must provide a freshly measured identity and the
//! current privacy revision before every observation or application effect.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationIdentity {
    /// Normalized executable identity, normally an absolute path.
    pub executable: String,
    /// Process identifier, paired with a creation identity by the adapter.
    pub pid: u32,
    /// Stable process-generation token supplied by the native adapter.
    pub process_generation: String,
    /// Top-level window handle represented without a platform type.
    pub window: i64,
    /// Stable window-generation token supplied by the native adapter.
    pub window_generation: String,
    /// Privacy revision observed when this identity was measured.
    pub privacy_revision: u64,
}

impl ApplicationIdentity {
    pub fn validate(&self) -> Result<(), &'static str> {
        if executable_key(&self.executable).is_none()
            || self.executable.len() > 32_767
            || self.pid == 0
            || self.process_generation.trim().is_empty()
            || self.process_generation.len() > 256
            || self.process_generation.chars().any(char::is_control)
            || self.window <= 0
            || self.window_generation.trim().is_empty()
            || self.window_generation.len() > 256
            || self.window_generation.chars().any(char::is_control)
        {
            return Err("invalid application identity");
        }
        Ok(())
    }

    pub fn same_target(&self, other: &Self) -> bool {
        self.validate().is_ok()
            && other.validate().is_ok()
            && executable_key(&self.executable) == executable_key(&other.executable)
            && self.pid == other.pid
            && self.process_generation == other.process_generation
            && self.window == other.window
            && self.window_generation == other.window_generation
    }
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationPrivacyPolicy {
    private_executables: BTreeSet<String>,
    private_processes: BTreeSet<String>,
    #[serde(default)]
    private_windows: BTreeSet<String>,
    #[serde(default)]
    private_documents: BTreeSet<String>,
}

impl std::fmt::Debug for ApplicationPrivacyPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApplicationPrivacyPolicy")
            .field("executable_count", &self.private_executables.len())
            .field("process_count", &self.private_processes.len())
            .field("window_count", &self.private_windows.len())
            .field("document_count", &self.private_documents.len())
            .finish()
    }
}

fn valid_token(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn window_key(identity: &ApplicationIdentity) -> String {
    // Hash only native identity, never a title, screenshot or accessible text.
    let bytes = serde_json::to_vec(&(
        executable_key(&identity.executable),
        identity.pid,
        &identity.process_generation,
        identity.window,
        &identity.window_generation,
    ))
    .expect("native identity tuple serialization");
    blake3::hash(&bytes).to_hex().to_string()
}

fn normalize(value: &str) -> String {
    value
        .replace('/', "\\")
        .trim_end_matches(['\\', ' '])
        .to_lowercase()
}

/// Conservative local executable spelling, independent of the host OS running
/// tests. Aliases must be resolved by a native probe; this does no filesystem I/O.
fn executable_key(value: &str) -> Option<String> {
    let path = value.replace('/', "\\");
    let bytes = path.as_bytes();
    if bytes.len() < 4
        || !bytes[0].is_ascii_alphabetic()
        || &bytes[1..3] != b":\\"
        || path.chars().any(char::is_control)
        || path
            .chars()
            .any(|c| matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
    {
        return None;
    }
    if path[3..].split('\\').any(|part| {
        part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with(['.', ' '])
            || part.contains(':')
    }) {
        return None;
    }
    Some(normalize(&path))
}

impl ApplicationPrivacyPolicy {
    /// Pre-launch executable eligibility only; no document read/effect grant.
    pub fn allows_executable(&self, executable: &str) -> bool {
        self.validate().is_ok()
            && executable_key(executable)
                .is_some_and(|key| !self.private_executables.contains(&key))
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if [
            &self.private_executables,
            &self.private_processes,
            &self.private_windows,
            &self.private_documents,
        ]
        .iter()
        .any(|set| set.len() > 256)
            || self
                .private_executables
                .iter()
                .any(|s| s.len() > 32_767 || executable_key(s).as_ref() != Some(s))
            || self.private_processes.iter().any(|s| !valid_token(s))
            || self
                .private_windows
                .iter()
                .chain(&self.private_documents)
                .any(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err("invalid application privacy rules");
        }
        Ok(())
    }

    pub fn private_executables(&self) -> Vec<String> {
        self.private_executables.iter().cloned().collect()
    }

    /// Strict UI/native input; never drop malformed entries and report success.
    pub fn replace_executables(&mut self, values: Vec<String>) -> Result<(), &'static str> {
        if values.len() > 256 {
            return Err("too many private applications");
        }
        let next = values
            .iter()
            .map(|s| {
                if s.len() > 32_767 {
                    return Err("invalid private executable");
                }
                executable_key(s).ok_or("invalid private executable")
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        self.private_executables = next;
        Ok(())
    }

    pub fn set_private_windows(
        &mut self,
        identities: &[ApplicationIdentity],
    ) -> Result<(), &'static str> {
        if identities.len() > 256 {
            return Err("too many private windows");
        }
        for identity in identities {
            identity.validate()?;
        }
        self.private_windows = identities.iter().map(window_key).collect();
        Ok(())
    }

    /// Opaque IDs must come from the adapter's native document binding, not titles.
    pub fn set_private_documents(&mut self, ids: &[String]) -> Result<(), &'static str> {
        if ids.len() > 256 || ids.iter().any(|s| !valid_token(s)) {
            return Err("invalid private document binding");
        }
        self.private_documents = ids
            .iter()
            .map(|s| blake3::hash(s.as_bytes()).to_hex().to_string())
            .collect();
        Ok(())
    }

    pub fn allows_document(
        &self,
        identity: &ApplicationIdentity,
        document_id: &str,
        revision: u64,
    ) -> bool {
        self.document_eligible(document_id) && self.allows(identity, revision)
    }
    pub fn document_eligible(&self, document_id: &str) -> bool {
        valid_token(document_id)
            && !self
                .private_documents
                .contains(blake3::hash(document_id.as_bytes()).to_hex().as_str())
    }
    pub fn set_private_executables<I, S>(&mut self, executables: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.private_executables = executables
            .into_iter()
            .map(|value| normalize(value.as_ref()))
            .collect();
    }

    /// Binds a private process to its measured generation, not just its PID.
    /// This prevents a reused PID from inheriting the old process's privacy
    /// decision.
    pub fn set_private_processes<I, S>(&mut self, processes: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.private_processes = processes
            .into_iter()
            .map(|value| value.as_ref().to_string())
            .collect();
    }

    /// Privacy eligibility only. This does not grant app observation or effects;
    /// ApplicationAuthority separately requires native consent and target scope.
    pub fn allows(&self, identity: &ApplicationIdentity, current_revision: u64) -> bool {
        if self.validate().is_err()
            || identity.validate().is_err()
            || identity.privacy_revision != current_revision
        {
            return false;
        }
        let executable = normalize(&identity.executable);
        !self.private_executables.contains(&executable)
            && !self.private_windows.contains(&window_key(identity))
            && !self
                .private_processes
                .contains(&identity.process_generation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_windows_bind_generation_and_documents_use_native_ids_not_titles() {
        let original = identity();
        let mut policy = ApplicationPrivacyPolicy::default();
        policy
            .set_private_windows(std::slice::from_ref(&original))
            .unwrap();
        assert!(!policy.allows(&original, 3));
        let mut reused = original.clone();
        reused.window_generation = "new-window".into();
        assert!(policy.allows(&reused, 3));
        policy
            .set_private_documents(&["native-document-canary".into()])
            .unwrap();
        assert!(!policy.allows_document(&reused, "native-document-canary", 3));
        assert!(policy.allows_document(&reused, "another-document-same-title", 3));
        policy
            .replace_executables(vec![r"C:\Other\tool.exe".into()])
            .unwrap();
        assert!(!policy.allows(&original, 3));
        assert!(!policy.allows_document(&reused, "native-document-canary", 3));
        assert!(!format!("{policy:?}").contains("canary"));
    }

    #[test]
    fn malformed_exclusions_are_rejected_instead_of_silently_dropped() {
        let mut policy = ApplicationPrivacyPolicy::default();
        for path in [
            "",
            "relative.exe",
            r"C:\Tools\..\private.exe",
            r"C:\Tools\private.exe:stream",
        ] {
            assert!(policy.replace_executables(vec![path.into()]).is_err());
        }
        assert!(policy
            .replace_executables(vec![r"C:\Tools\ok.exe".into(); 257])
            .is_err());
        policy.set_private_executables(["relative.exe"]);
        assert!(policy.validate().is_err());
        assert!(!policy.allows(&identity(), 3));
        assert!(policy.set_private_documents(&["\0".into()]).is_err());
    }

    #[test]
    fn rejects_ambiguous_executable_spellings_before_any_measurement() {
        for executable in [
            "editor.exe",
            r"\\server\editor.exe",
            r"C:\Tools\..\editor.exe",
            r"C:\Tools\editor.exe:stream",
            "C:\\Tools\\editor.exe ",
            "C:\\Tools\\bad\0.exe",
        ] {
            let mut invalid = identity();
            invalid.executable = executable.into();
            assert!(
                invalid.validate().is_err(),
                "unexpected accepted executable spelling"
            );
        }
        let mut same = identity();
        same.executable = r"c:\Program Files\Editor\editor.exe".into();
        assert!(identity().same_target(&same));
    }

    fn identity() -> ApplicationIdentity {
        ApplicationIdentity {
            executable: r"C:/Program Files/Editor/editor.exe".into(),
            pid: 42,
            process_generation: "process-42-start-1".into(),
            window: 7,
            window_generation: "window-7-create-1".into(),
            privacy_revision: 3,
        }
    }

    #[test]
    fn private_executable_is_refused_before_observation() {
        let mut policy = ApplicationPrivacyPolicy::default();
        policy.set_private_executables([r"c:\program files\editor\editor.exe"]);
        assert!(!policy.allows(&identity(), 3));
    }

    #[test]
    fn private_process_binding_is_generation_specific() {
        let mut policy = ApplicationPrivacyPolicy::default();
        policy.set_private_processes(["process-42-start-1"]);
        assert!(!policy.allows(&identity(), 3));

        let mut replacement = identity();
        replacement.process_generation = "process-42-start-2".into();
        assert!(policy.allows(&replacement, 3));
    }

    #[test]
    fn stale_privacy_revision_is_refused() {
        let policy = ApplicationPrivacyPolicy::default();
        assert!(!policy.allows(&identity(), 4));
    }

    #[test]
    fn malformed_identity_is_refused() {
        let policy = ApplicationPrivacyPolicy::default();
        let mut invalid = identity();
        invalid.pid = 0;
        assert!(!policy.allows(&invalid, 3));
    }
}
