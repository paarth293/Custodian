/// Status of system backup verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupStatus {
    Available,
    NotConfigured,
    Unavailable,
}

/// Reports whether a usable backup exists.
///
/// F29: Replaced placeholder constant `true`. Until File History or OneDrive
/// detection is integrated, reports `BackupStatus::Unavailable` so callers
/// do not treat absent backup systems as confirmed safe.
pub fn check_backup_status() -> BackupStatus {
    BackupStatus::Unavailable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_status_reports_unavailable_rather_than_false_safe() {
        assert_eq!(check_backup_status(), BackupStatus::Unavailable);
    }
}
