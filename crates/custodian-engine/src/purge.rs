const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

#[derive(Debug, PartialEq)]
pub struct PurgeCandidate {
    pub id: String,
    /// Unix timestamp in seconds.
    pub timestamp: u64,
}

/// Returns the ids of records older than the retention window.
pub fn get_purge_candidates(
    records: &[PurgeCandidate],
    current_time: u64,
    retention_days: u64,
) -> Vec<String> {
    let cutoff = current_time.saturating_sub(retention_days * SECONDS_PER_DAY);
    records
        .iter()
        .filter(|r| r.timestamp < cutoff)
        .map(|r| r.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_only_expired_records() {
        let now = 1_000_000_000;
        let records = vec![
            PurgeCandidate {
                id: "old1".into(),
                timestamp: now - 31 * SECONDS_PER_DAY,
            },
            PurgeCandidate {
                id: "new1".into(),
                timestamp: now - 10 * SECONDS_PER_DAY,
            },
        ];

        assert_eq!(
            get_purge_candidates(&records, now, 30),
            vec!["old1".to_string()]
        );
    }
}
