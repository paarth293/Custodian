# Stage 1: authority and recovery

Status: Stage 1 accepted on the reference Windows machine. The optimized desktop
passed all 27 designated live checks and exited its fixture processes cleanly.
This increment implements Stage 1 only. It does not enable general app control,
document editing, scripts or online tools, and does not declare the whole product
accepted. The Stage 0 baseline and its installer remain historical evidence.

## Execution boundary

Every exposed native ToolCall is classified exhaustively by one contract in
`assistant/capability.rs`: typed input, resource class, effect, permission,
precondition, verifier, cancellation boundary and recovery behavior. Unknown
model tool names fail parsing. The dispatcher checks live privacy and Stop
before preparation and again before executing; approved file plans also pass
this check. File executors still enforce exact objects, hashes, no replacement
and sealed receipts at commit. Settings share the privacy mutation boundary and
retain their catalogue, readback and history checks. Launch dispatch is reported
as opening an application, never as completed work inside it.

Ordinary approved copies, cross-drive moves and cross-drive move undo now also
check the destination's measured capacity and keep at least 1 GiB free. Exact
approval does not waive that floor. Same-drive renames and removing a verified
copy do not duplicate content and retain their existing checks. This is a
preflight budget, not an atomic reservation against space consumed by other
programs. The stricter standing-workflow budgets still apply as well.

Standing backup and collection operations use a native copy-only grant. The
operation is fixed by the trusted user-goal parser, not model arguments. The
grant binds source and destination scopes and revision before discovery and at
each copy. The existing size, free-space, file-count, private-path, protected-path
and no-overwrite constraints remain mandatory. These grants do not authorize
moves, deletion, settings or app control.

The model still proposes bounded collection operators; it cannot create consent
or edit grants. Laya remains experimental and inactive. This increment makes no
new claim about general planning quality.

## Whole-system scope and standing permission

The source and destination lists in Routine permissions are optional. An empty
list means any permitted local location explicitly named in a user request,
including all local drives for discovery. A populated list restricts that side
to the named directories and descendants. This preserves the existing broad
opt-in scope and permits narrower grants without requesting approval for every
file. A request outside a restricted scope fails before file discovery.

Permissions remain off by default. Existing single-file permission never
enables collection permission. Source and destination restrictions are sealed
alongside the two operation grants. Changes invalidate the captured revision;
queued requests must be submitted again. No model input sets a grant.

## Stop and revocation

The host captures the privacy revision, standing-grant revision and request
epoch before queuing work. Stop advances the epoch, cancels registered tasks and
clears pending approvals. A task cannot register after Stop's cancellation pass
under old authority. A later chat request may answer during an independent scan
but cannot revive the stopped worker. Nested helpers retain the original epoch
and privacy revision. Approval cards retain their original privacy revision,
including publication racing with a privacy change.

Discovery and model work observe cancellation. A dispatched atomic file or
settings commit may finish its verification and receipt; stopping does not tear
it down midway. Collections check cancellation before subsequent copies. A
permission change waits for the current copy's safe commit boundary and then
invalidates subsequent steps. The UI describes this behavior.

## One authoritative host

Before loading mutable policies, tasks or receipts, the desktop holds an OS file
lock in a fixed OS-user authority directory, which ignores verification-profile
overrides. Directory handles pin the authority and state paths against ordinary
rename/replacement on Windows. A second host for the same user exits before
loading mutable state, even when using a different verification profile. Profiles
retain separate data but cannot operate concurrently. Process death releases the lock without trusting a PID file
or deleting a stale lock. This coordinates desktop hosts; it is not a protection
against privileged software or hostile programs already running as this user.

## Private observations

Assistant screenshot capture, accessibility reading and app-content extraction
have no registered model or native command route. Window-title enumeration,
broad app actions and legacy cleanup remain paused at the shared gate, including
for approved origins. This enforces refusal before observation; it is not an
implementation of selective screenshot redaction or general app private zones.
Custodian's existing own-window screen-sharing protection is a different feature.

Supported file reads continue to use the live private-path policy. Folder item
counts now exclude protected/private children. Task listing filters description,
progress and resources, rather than description alone. Old sealed task/receipt
records are retained for recovery; no general history/index query is enabled.
Existing third-party applications are not network-isolated by these controls.

## Failure and recovery evidence

The process-death suite launches the actual Rust test executable, executes real
workflow/file stores in disposable folders, waits for a deterministic barrier,
kills that exact child process and reloads sealed state. Four barriers cover:

1. Prepared child receipt, before any filesystem mutation.
2. First 1 MiB copied, with a partial destination and prepared receipt.
3. Completed child receipt persisted, before publication to its parent step.
4. All copies verified, before the parent completion record is published.

Restart inspection never replays steps. It verifies actual child receipts and
bytes, reports incomplete parents as needing recovery, retains ambiguous
partial files and permits inverse operations only for verified completed copies
with unchanged originals. Fault barriers are compiled only into tests; release
binaries have no environment-controlled crash or fault command.

An ERROR_DISK_FULL failure is injected after writing a real partial destination.
The source and partial output remain, a recovery receipt is persisted, and undo
refuses to delete unverified output. This tests the error path; it does not claim
that a real disk was filled, that physical media failed, or that receipts can be
persisted on an already-full state volume. Unavailable destinations, corrupt
receipts, changed sources, replaced paths and stale permissions have separate
real-fixture checks. Corrupt existing file-task history now blocks new mutations.

## Validation record

The final source passes 361 native tests (four opt-in environment tests ignored)
and 131 JavaScript tests. Strict workspace Clippy, frontend lint and targeted
formatting pass. The frozen Stage 0 expectation set remains unchanged. Raw
evidence is kept in `docs/EVIDENCE/stage-1/`.

| Boundary | Executed native evidence |
| --- | --- |
| Second host, crash release, state-path replacement | `instance::tests::second_process_is_excluded_and_killed_owner_releases_authority` |
| Linked lock refusal | `instance::tests::linked_lock_file_is_rejected_before_state_can_be_loaded` |
| Alternate profiles cannot bypass user authority | `instance::tests::alternate_profiles_share_one_authority_lease` |
| Queued privacy revision and nested helpers | `privacy::tests::queued_revision_cannot_be_refreshed_by_worker_or_nested_helper` |
| Stop and queued admission | `assistant::tests::stop_and_privacy_changes_invalidate_queued_requests_before_they_can_clear_stop` |
| Later chat cannot revive old tasks; private task resources hidden | `assistant::tests::new_chat_does_not_revive_stopped_task_or_expose_newly_private_task_resources` |
| Late approval publication | `assistant::actions::tests::late_published_cards_keep_original_privacy_revision` |
| Model consent refusal and paused routes | `assistant::capability::tests::consent_and_paused_routes_cannot_be_granted_by_model_arguments` |
| Screen/app/script routes absent | `assistant::capability::tests::app_and_screen_capture_and_scripts_have_no_model_dispatch_route` |
| Source/destination grant scope and revision | `routine::tests::scoped_grants_bind_operation_resources_and_revision_before_discovery_and_commit` |
| Revocation at the safe commit boundary | `routine::tests::revocation_waits_for_mutation_without_blocking_request_revision_capture` |
| Private folder counts | `assistant::tests::folder_counts_exclude_private_children_and_credentials` |
| Absolute Windows executable/package dispatch | `assistant::tests::catalogued_launches_resolve_absolute_protected_windows_paths` |
| Four actual process-kill boundaries | `crash_probe::actual_process_death_at_write_boundaries_is_reconciled_without_replay` |
| Injected disk-full after a real partial write | `crash_probe::injected_disk_full_after_real_write_preserves_partial_file_and_recovery_receipt` |
| Exact approval cannot waive destination reserve | `crash_probe::exact_approval_cannot_bypass_capacity_reserve_before_mutation` (capacity measurement injected; real executor and files) |
| Unavailable destination and corrupt prior journal | `file_tasks::tests::unavailable_destination_and_corrupt_history_refuse_new_mutations` |
| Changed files and replaced source/destination paths | Existing `file_tasks::tests::approval_rejects_*` and `undo_refuses_*` cases |

Earlier failures and superseded checks are retained. In particular, the first
crash-test expectation incorrectly assumed undo would accept a prepared partial
receipt; the corrected test requires refusal. A later launcher test caught that
this PC supplies Paint only as a Microsoft package. Its manifest identified
`Microsoft.Paint_8wekyb3d8bbwe!App`; the fixed launcher uses that constant through
absolute trusted Explorer, with no app enumeration or executable search.

Superseded production builds were stopped after final review changes, before
acceptance. The final build and production desktop run passed. Final
artifact identities are recorded in `docs/EVIDENCE/stage-1/artifacts.json`. The installer remains the
Stage 0 artifact; no Stage 1 installer has been built or executed.

The first embedded-assets development desktop walkthrough reached the scoped
permission checks, the first model-planned collection and its conflict refusal,
but failed while waiting for a captured WebView process during a planned restart.
That run is not accepted. Cleanup now snapshots the descendant chain before
termination, checks creation times against each parent to exclude recycled PID
relationships, and terminates/waits only through pinned kernel handles. It
attempts cleanup for every captured descendant before reporting failures and
retains completed-check/failure text in the fixture. The failed run is retained
separately from the subsequent complete passing runs.

The fresh development executable then passed all 27 checks using the corrected
harness (`desktop-validation-02.txt`). This includes the model's real document,
photo and largest-document workflows, changed-output preservation, cross-drive
file work, whole-system discovery, privacy, scope persistence and cross-profile
admission. Its SHA-256 is
`274326B9ECF457D4A6B478648080295B1F8C5502B82CE33E6A74EC4F5DAE64FB`.
It is an embedded-assets development build, not the final optimized artifact.
The final optimized executable also passed its own complete acceptance run.

The optimized build completed with `tauri/custom-protocol` and embedded assets.
Its size is 6,177,792 bytes and SHA-256 is
`FAFBF74E466E719E4AD6784484798D73F63DAD87BC8A37A7666A005DD96237A4`.
Its live acceptance run passed all 27 checks (`desktop-production-01.txt`),
including process cleanup. Static PE inspection reports x64, ASLR and
DEP enabled; the Guard CF flag is absent. Test-only crash/lock child environment
keys are absent from the executable. This is an unsigned development artifact;
compiler hardening, code signing and installed-product acceptance are not closed
by this authority/recovery increment. The historical Stage 0 installer does not
contain the Stage 1 changes.

The accepted source baseline is `docs/EVIDENCE/stage-0/stage-1-accepted.json`
(the existing baseline capture helper retains its Stage 0 directory name).
All 318 selected source/config/asset files, totaling 2,125,328 bytes, were
reconstructed in a fresh directory and verified byte-for-byte against the
snapshot and current source. A second snapshot copy is preserved outside
`target`; its SHA-256 is
`056f84ccf1d92452c25ab56bd8d4ed590370ec5dd302e5b1c490605e82ae3a2c`.
The frozen expectation hash remains
`20d15ee73ce2b261a4905fd303776cdc7682f158f72686fe84a015e5cbc1d281`.
This is a selected dirty-source restore point, not a self-contained dependency
bundle or certified bit-for-bit compiler rebuild. The original baseline and
failed/superseded evidence were preserved. Stage 2 has not started.
