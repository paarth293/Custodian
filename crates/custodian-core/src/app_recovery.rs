//! Read-only reconciliation of task-owned app output. No application input,
//! replay, publication, overwrite or permission restoration exists here.
use crate::{
    app_journal::{ApplicationJournal, Event, ExpectedArtifact},
    file_tasks::FileTasks,
};
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};
fn invalid() -> io::Error {
    io::Error::other("Application recovery evidence requires manual review")
}
fn bounded(path: &Path) -> io::Result<Vec<u8>> {
    let _pin = custodian_winsec::guarded_copy::CopyGuard::open(
        path,
        &path.with_extension("inspection-only"),
    )?;
    let mut bytes = vec![];
    File::open(path)?
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(invalid());
    }
    Ok(bytes)
}
/// The native host must first admit the task's document, app and file scope.
/// Returned strings contain states only, never document text or window titles.
pub fn inspect(workspace: &Path, document_id: &str, files: &FileTasks) -> io::Result<String> {
    crate::privacy::check(workspace)?;
    let _directory = custodian_winsec::guarded_copy::DirectoryGuard::open(workspace)?;
    let evidence = ApplicationJournal::recover(&workspace.join("journal"))?;
    let scope = evidence
        .events
        .iter()
        .find_map(|e| {
            if let Event::Scope { scope } = e {
                Some(scope)
            } else {
                None
            }
        })
        .ok_or_else(invalid)?;
    if document_id
        != format!(
            "file-document:{}:{}",
            scope.source_identity.volume, scope.source_identity.index
        )
    {
        return Err(invalid());
    }
    crate::privacy::check(&scope.source)?;
    crate::privacy::check(&scope.destination)?;
    let source = if custodian_winsec::fsops::file_identity(&scope.source).ok()
        == Some(scope.source_identity)
        && bounded(&scope.source).is_ok_and(|b| blake3::hash(&b).as_bytes() == &scope.source_digest)
    {
        "original identity and bytes unchanged"
    } else {
        "changed or unavailable; preserve the current file"
    };
    let stage = workspace.join(match scope.expected {
        ExpectedArtifact::Text { .. } => "document.txt",
        ExpectedArtifact::Workbook { .. } => "output.xlsx",
    });
    crate::privacy::check(&stage)?;
    let stage_state = if !stage.try_exists()? {
        "not present"
    } else if bounded(&stage).is_ok_and(|bytes| scope.expected.matches(&bytes)) {
        "matches the approved result"
    } else {
        "changed or unverified; preserved for manual review"
    };
    let publication_id = evidence.events.iter().rev().find_map(|e| match e {
        Event::Published { receipt_id } | Event::PublishPending { receipt_id } => Some(receipt_id),
        _ => None,
    });
    let publication = if let Some(id) = publication_id {
        let receipts = files.receipts()?;
        match receipts.iter().find(|r| &r.id == id) {
            Some(receipt)
                if receipt.destination == scope.destination
                    && receipt.source.as_deref() == Some(&stage)
                    && files.verify_copy(receipt).is_ok() =>
            {
                "verified destination and receipt; verified file undo is available"
            }
            Some(receipt)
                if receipt.destination == scope.destination
                    && receipt.source.as_deref() == Some(&stage)
                    && receipt.state == crate::file_tasks::ReceiptState::Undone =>
            {
                "verified publication was undone"
            }
            _ => "unverified or changed; preserve existing files and inspect the file receipt",
        }
    } else if scope.destination.try_exists()? {
        "no publication receipt; existing destination was preserved"
    } else {
        "not published"
    };
    let process = evidence.events.iter().rev().find_map(|e| {
        if let Event::OwnedProcess { pid, birth } = e {
            Some((*pid, *birth))
        } else {
            None
        }
    });
    let app = if process.is_some_and(|(pid, birth)| {
        custodian_winsec::apps::process_is_running(pid, birth).unwrap_or(true)
    }) {
        "recorded instance may still be running; close it normally in its own window if needed"
    } else {
        "recorded instance is no longer running"
    };
    Ok(format!("Application recovery inspection\nSource: {source}.\nSaved task output: {stage_state}.\nPublication: {publication}.\nApplication: {app}.\nJournal: {}; {}. No application action was replayed.",
        if evidence.completed { "completion was recorded" } else { "completion was not recorded" },
        if evidence.uncertain_effect { "an effect was interrupted or its receipt is missing" } else { "no pending effect is recorded" }))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::app_journal::{ExpectedArtifact, RecoveryScope};
    #[test]
    fn missing_save_receipt_is_reconciled_read_only_and_later_edits_are_preserved() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("task");
        std::fs::create_dir(&workspace).unwrap();
        let source = temp.path().join("source.txt");
        let destination = temp.path().join("new.txt");
        let stage = workspace.join("document.txt");
        std::fs::write(&source, b"original").unwrap();
        std::fs::write(&stage, b"expected edit").unwrap();
        let identity = custodian_winsec::fsops::file_identity(&source).unwrap();
        let document_id = format!("file-document:{}:{}", identity.volume, identity.index);
        let journal = ApplicationJournal::create(&workspace.join("journal"), [1; 32]).unwrap();
        journal
            .append(Event::Scope {
                scope: RecoveryScope {
                    source: source.clone(),
                    destination: destination.clone(),
                    source_identity: identity,
                    source_digest: *blake3::hash(b"original").as_bytes(),
                    expected: ExpectedArtifact::Text {
                        digest: *blake3::hash(b"expected edit").as_bytes(),
                        bytes: 13,
                    },
                },
            })
            .unwrap();
        journal
            .append(Event::Pending {
                nonce: uuid::Uuid::new_v4().to_string(),
                request_digest: [9; 32],
            })
            .unwrap();
        drop(journal);
        let files = FileTasks::new(temp.path().join("file-receipts"));
        for _ in 0..2 {
            let result = inspect(&workspace, &document_id, &files).unwrap();
            assert!(result.contains("matches the approved result"));
            assert!(result.contains("receipt is missing"));
            assert!(!destination.exists());
            assert_eq!(std::fs::read(&stage).unwrap(), b"expected edit");
        }
        std::fs::write(&stage, b"later user edit").unwrap();
        let result = inspect(&workspace, &document_id, &files).unwrap();
        assert!(result.contains("changed or unverified"));
        assert_eq!(std::fs::read(&stage).unwrap(), b"later user edit");
        assert_eq!(std::fs::read(&source).unwrap(), b"original");
        assert!(inspect(&workspace, "different-native-document", &files).is_err());
    }
}
