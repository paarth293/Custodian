# Stage 5 execution record

Date: 18 October 2026. This record is intentionally retained in the
repository so an interrupted implementation can resume without treating code
presence or an earlier claim as acceptance evidence.

## Prerequisite

Stage 4 is **not accepted**. The Stage 4 result records a verified native
foundation increment, but real Notepad/Excel adapters, live app observation,
optimized Stage 4 packaging and clean-install acceptance remain open. Stage 5
file work is independently actionable; no Stage 5 claim upgrades Stage 4.

## Implementation map

| Stage 5 area | Implementation | Current state | Required evidence still open |
|---|---|---|---|
| 5.0 exact manifest | `crates/custodian-core/src/batch_archive.rs` (`BatchManifest`, snapshots, expected states, exclusions) | implemented and unit-tested | native planner/permission-card route and full parent-private-descendant suite |
| 5.1 ordinary file primitives | existing `file_tasks.rs`; exact approval, identity/hash checks, journal receipts and undo | existing verified subset; shared multi-item organization executor is not connected | duplicate/case collision, rename-cycle, cross-volume and process-kill acceptance through product route |
| 5.2 ZIP | `batch_archive.rs` bounded ZIP writer/parser, staging extraction and independent reopen checks | implemented and unit-tested; not exposed in desktop route | optimized package route, crash-at-publication evidence, Windows hostile archive matrix |
| 5.3 backup | `versioned_backup.rs` immutable owned versions, state/integrity manifests, retention | implemented and unit-tested; native authority/route not connected | real independent restore and interruption/drive-loss acceptance in packaged product |
| 5.4 restore | `BackupStore::prepare_restore*`, staged new-location restore and tree verifier | implemented and unit-tested | original-location overwrite authority and actual process termination drill |
| 5.5 recycle/cleanup | `governed_recycle.rs`, separate approval digest and `RecoveryProvider` | governed API and fake-provider tests; Windows provider is deliberately unsupported | qualified Windows Recycle Bin provider, native receipts, actual restore drill |
| 5.6 boundaries | ordinary local non-reparse files, privacy checks, capacity/size ceilings | refusal boundaries covered in modules | supported-volume matrix for C:/D:, removable loss, locked/read-only/long-path cases |
| 5.7 delivery | this record plus stage evidence files | in progress | native routes, UI cards, package smoke test, hashes and full acceptance report |

## Commands run

- `cargo check -p custodian-core --locked` — passed.
- `cargo test -p custodian-core --lib --locked` — passed (198 passed, 2 ignored).
- `cargo test --workspace --locked` — passed; the complete log is retained at
  `target/stage5-workspace-tests.txt` (workspace ignored tests remain ignored).
- The focused standalone harness was removed after the modules were exported;
  their 12 backup/recycle tests now run as part of the core library suite.
- `cargo clippy -p custodian-core --all-targets --locked -- -D warnings` — passed.
- `cargo fmt --all -- --check` — not clean because unrelated existing Stage 4
  worktree files are not formatted; the new module code was formatted.

## Exact blockers

1. Stage 4 prerequisite acceptance is open.
2. The new backup and archive APIs are core primitives, not yet connected to
   desktop language routes, native consequence-specific permission cards, or
   task coordinator receipts.
3. No qualified Windows Recycle Bin provider exists. The safe default refuses
   recycle rather than unlinking.
4. This environment has not provided a disposable optimized-package Windows
   run, removable-drive loss test, or real process termination at the new
   archive/backup publication boundaries.

## Continuation order

1. Add a Stage 5 capability registry to the native planner and coordinator;
   make batch manifests the only input to archive/backup/recycle effects.
2. Add desktop command/parser/permission-card routes and persist receipts in
   the existing history/journal path.
3. Implement and qualify a Windows-only Recycle Bin provider; until then keep
   F09 unavailable.
4. Freeze C:/D:/custom-volume fixtures and run optimized packaged acceptance,
   including actual process kills and restart reconciliation.

Stage 5 remains **incomplete** until all mandatory routes and recovery gates
are demonstrated. The core module tests are implementation evidence only.
