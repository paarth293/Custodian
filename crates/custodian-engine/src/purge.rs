#[derive(Debug, PartialEq)]
pub struct PurgeCandidate {
    pub id: String,
    pub timestamp: u64,
}

pub fn get_purge_candidates(
    records: &[PurgeCandidate],
    current_time: u64,
    retention_days: u64,
) -> Vec<String> {
    let retention_secs = retention_days * 24 * 60 * 60;
    let cutoff = current_time.saturating_sub(retention_secs);
    
    records.iter()
        .filter(|r| r.timestamp < cutoff)
        .map(|r| r.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_purge_rules() {
        let now = 1000000000;
        let day = 24 * 60 * 60;
        
        let records = vec![
            PurgeCandidate { id: "old1".into(), timestamp: now - (31 * day) },
            PurgeCandidate { id: "new1".into(), timestamp: now - (10 * day) },
        ];
        
        let candidates = get_purge_candidates(&records, now, 30);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], "old1");
    }
}
