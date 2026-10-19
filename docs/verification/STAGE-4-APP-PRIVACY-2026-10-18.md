# Stage 4: persistent native application privacy

18 October 2026. Incremental follow-up to the supervised worker foundation.
Stage 4 remains open. This adds settings and admission enforcement, not an
enabled content reader or a real editor/spreadsheet adapter.

## Implemented behavior

The desktop loads a separate `app-privacy.dpapi` policy from its native data
directory before making assistant state available. Fresh profiles initialize an
empty exclusion policy with revision zero; this grants no application access.
Each app operation still needs an independently issued native grant, a current
target/document binding and a qualified worker adapter. Production worker
remeasure/execute operations continue to return unavailable.

The Privacy and capabilities panel now accepts full executable paths for a
Never access these applications list. Entering an exclusion performs no app
launch, executable read, window enumeration, title read or document observation.
Existing programs need not be installed to record their exact path. Saving
normalizes valid local spellings and rejects relative, wildcard, stream and
ambiguous paths. The list can be edited and removed; saving uses the displayed
revision to reject stale updates. Errors require a reload of the last saved
settings instead of displaying unsaved edits as accepted policy.

An application exclusion is a deny rule for application adapters. It does not
disable the program, isolate its network, hide its files from filesystem tools,
or grant access to other programs. The panel explicitly directs users to private
file locations for file-tool exclusions. No Documents/Downloads-only operating
boundary was introduced.

The policy also supports native process-generation, exact window-generation and
opaque document-ID exclusions. Window keys bind executable, PID/process
generation and window handle/generation; document keys use native IDs, never a
display title. Window/document keys are hashed before storage. Executable-only UI
updates preserve these other exclusions. Native tests establish these rules;
the UI does not yet select live windows/documents, and native ID validity in real
applications remains an adapter gate. Exact path matching does not establish
file identity across every alias, rename or version. Content observation remains
disabled until the adapter can enforce this before reading anything.

Privacy eligibility and positive authority share the same live lock and revision.
Updates invalidate earlier grants, including worker watchdog checks. A restart
creates a fresh random authority issuer, so no prior grant is restored even
when its old request matches the saved revision. Request validation consults
document exclusions before calling an adapter's remeasurement method.
The worker integration fixture also saves a private executable policy while an
authenticated worker is already running: the host detects the changed authority,
terminates that worker and never reaches the effect checkpoint. This is checked
separately from directly revoking a grant.

## Persistence and failure behavior

The store uses Windows per-user DPAPI. A strict versioned JSON schema is validated
after decryption. Read/decrypt payloads are bounded to 512 KiB; each exclusion
class is limited to 256 entries. A required marker is created/synced before the
initial usable policy. Missing policy with that marker, orphan policy without a
marker, corrupt marker/blob, unknown schema, invalid entries and oversized data
stop loading instead of restoring empty defaults. Interrupted initialization
also requires explicit repair; it never silently permits app access.

A native exclusive file handle prevents simultaneous owners of a store.
Changes write a fresh temporary file, flush it, and replace the native settings
file with Windows write-through semantics before publishing the new live policy.
Failed commits pause all app grants for that authority until restart and preserve
the previous live revision. Temporary cleanup targets only the file created by
that invocation. Known reparse/cloud/symlink ancestors and storage objects are
refused. This is not a defense against an attacker already controlling the same
Windows account, valid-blob rollback, deletion of both policy and marker, or an
arbitrary OS/filesystem failure. No release signing or universal power-loss
durability guarantee is claimed.

Saving app rules stops queued/current tasks, clears approvals/native context and
the displayed conversation. The model process checks both file-privacy and
app-privacy revisions before accepting the next turn, discarding its previous
process when either changed. No private app content is currently supplied to
the model. These controls prepare that boundary without enabling observation.

## Verification

Commands, test counts, exact development artifact identity and the desktop UI
walkthrough are recorded separately in
[privacy-result.json](../EVIDENCE/stage-4/privacy-result.json). Logs are prefixed
`privacy-` in the same evidence directory. The earlier foundation and worker
results/source snapshots remain historical and are preserved.

Native cases cover encrypted round trips, restart and issuer invalidation,
stale/competing revisions, exclusive ownership, failed replacement, corruption,
oversized/unknown saved policy, window reuse, opaque documents and preservation
of session/window/document exclusions during an executable-only update. Existing
worker and file/AppContainer regression tests remain part of workspace checks.
The desktop fixture targets only its own newly launched verification-profile
process/window and disposable state. It saves an exclusion, checks conversation
clearing, restarts, refuses malformed input without altering the saved blob,
reloads, removes the exclusion and restarts again.

Final workspace regression: **431 native tests passed, 0 failed, 7 ignored**;
**138 JavaScript tests passed**. Strict Clippy, TypeScript checking, lint and the
frontend build pass. The four desktop walkthrough checks pass with exit zero,
including exact-handle cleanup of each fixture host and captured descendants.
The worker fault matrix is explicitly rerun separately from the default suite.
Its first rerun is retained: a cold-copy fixture missed its old two-second
deadline before recording startup. That case was not accepted as coverage.
The updated harness uses ten-second operation deadlines, including startup,
and still requires a PID/creation record, the expected failure class, confirmed
termination and an initial-call duration below sixteen seconds including
cleanup. This changes test qualification timing, not production timeout limits.
The other ignored tests retain their historical requirements; no ignored test
is counted as a default-suite pass. The frozen register audit still verifies
58 requirements, 17 source routes and 15 frozen Stage 0 cases.

This increment builds a development desktop with embedded frontend assets;
the accepted Stage 3 optimized desktop, earlier optimized worker and historical
Stage 0 installer retain their own identities. No new production installer or
installation acceptance is claimed. Docker remains deferred by the user;
Laya remains inactive. No Stage 5 or 6 work starts here.

## Next gate

Qualify native metadata/document admission for one dedicated editor session and
its exact installed version. Establish current document identity and private-path
bindings before any content read; unknown or mixed/tabbed contexts must remain
unread. Then implement bounded open/inspect/edit/save/normal-close operations,
independent saved-byte verification and durable pending/receipt recovery. The
persisted settings and synthetic worker tests do not satisfy these real-app gates.
