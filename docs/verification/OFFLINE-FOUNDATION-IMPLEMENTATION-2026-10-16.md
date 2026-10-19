# Offline foundation implementation — 16 October 2026

This is the first implemented package from the offline master plan. It does
not complete the general desktop operator or certify the entire M1/M2 gates.
Older reports and installers describe their earlier source snapshots.

The installer at the output path in this historical report has since been
replaced by the [17 October workflow increment](MODEL-PLANNED-WORKFLOWS-2026-10-17.md).
Use that report for current artifact hashes and additional capability evidence;
the sizes, hashes and counts below describe the earlier foundation build.

## What changed

- A shared Rust filesystem policy checks supported live file operations and
  discovery before acquiring ordinary metadata/content. Native commands bind
  the configured policy to their worker; default credential restrictions also
  apply to core discovery and hashing.
- Native privacy controls let the user mark existing absolute local files or
  folders as private. Rules are DPAPI-sealed and persisted before publication.
  A marker makes a missing previously saved policy fail startup; malformed or
  tampered saved policy also fails startup. This is not protection against
  arbitrary same-user code deleting both policy and marker.
- Path and object checks cover case/volume normalization, ancestor reparse/cloud
  refusal, known private identities after renaming, and conservative rejection
  of multi-linked files. Minimal OS identity checks are policy resolution, not
  data exported to the model. Ambiguous/unsupported access fails closed.
- Common credential paths/file names and the application's policy-storage area
  are excluded from supported user-file tools. This is a bounded default list,
  not a detector for every possible secret in arbitrary files.
- Config changes stop active tasks, clear pending approvals/plans/references and
  context, reject stale preference updates, invalidate prepared file plans and
  prevent reuse of old model sessions. Old-generation streamed output and late
  approval responses are suppressed. Noninterruptible file mutations finish
  before a rule update commits; the UI explains that boundary.
- File execution and undo check current policy, including source/destination,
  before acting. Exact source/content and destination checks remain in place.
  Opening a file or known folder also checks the policy before handing it to
  Windows; a private Documents folder cannot bypass the rules via its shortcut.
- Online search/site handoffs are denied before execution even for approved
  calls. Their model tools are unavailable; an offline request cannot silently
  open a browser or obtain online permission from model text.
- The UI and “what can you do?” expose actual available, limited, paused,
  deferred and not-implemented capabilities.
- Browser helpers no longer claim execution without a browser. The reader
  reports extraction unavailable instead of returning a pretend extraction.
  The disconnected orchestrator IPC activity now errors instead of reporting
  a successful send after logging.

## Intentional temporary restrictions

Legacy cleanup/temp deletion/purge/history, broad installed-app launch, window
listing/closing and checkup suggestions are paused because they use a separate
unmigrated observation/execution path. Automatic safe-place purge is disabled;
existing receipts/data are retained. Legacy cleanup undo is paused; verified
file-task undo remains enabled. These restrictions are visible, not represented
as newly implemented capabilities.

The six personal settings, fixed built-in app launch, local chat, arithmetic,
reminders and permitted file tools remain available with their existing limits.
App-level private zones, general app observation/control and content-writing
capabilities are not implemented by this package.

## Architecture decision for this package

Use `custodian-core::privacy::AccessPolicy` as the shared file-policy authority,
bound explicitly at live Rust worker entries. Thread-local binding restores on
normal exit and panic; spawned workers must be bound explicitly. Policy is
shared across active workers, not copied into an immutable stale scan snapshot.

Policy updates and verified single-file mutation execution serialize through a
separate operation lock. Readers check current rules as they traverse; request
generation fences suppress observations from revoked requests. Core policy
tests use only newly created fixtures.

This does not replace the planned dedicated Engine process or silently revise
Temporal/PostgreSQL/framework decisions. Settings, reminders and broader task
authority still need their respective execution-contract work. Do not expose
new readers, app automation or commands by assuming this initial policy slice
already closes every boundary in the master plan.

## Verification

Fresh core tests cover:

1. Private-vs-public sibling paths, default secret exclusions and context filtering.
2. Hard-link aliases and renamed private folders.
3. Sealed policy reopen, tampering and missing saved state.
4. Revoked output revisions and restoration of worker policy after panic.
5. Actual largest-file discovery, hashing, stale approved move refusal and
   refusal to undo into/from newly private data.
6. Stale preference updates cannot overwrite newer rules.
7. Revoked worker requests cannot regain authority through nested helper binding;
   a new request sees the new policy.

Desktop regressions cover unavailable/approved-call boundaries and removal of
previously permitted private paths from the model-context projection. Agent
tests ensure deferred online requests execute no tool and do not load the
model. Browser tests require explicit unavailability, never fake execution.

Final packaged live UI evidence: `target/offline-foundation-live-recheck.log`,
using a fresh marked profile `desktop-acceptance-privacy-05` under the session
artifact directory and
`D:\Custodian\fixture\desktop-acceptance-privacy-destination-05` as the other-drive
destination. All eight checks passed: private Documents opening refusal,
native private-zone configuration, measured search with
a larger excluded private canary, declined approval, approved C:-to-D: move,
restart/verified undo, persisted exclusions, offline web-handoff refusal and a
C:/D: scan with an independent answer and Stop. Only synthetic files were
changed. An earlier harness attempt addressed a disabled input before the
async settings load finished; the harness now waits for an enabled input.
The final build's first run (`target/offline-foundation-live-final.log`, profile
04) timed out waiting for that input; its captured UI showed the panel still
collapsed. The unchanged rerun passed. Treat this as an intermittent UI/automation
interaction failure, not as evidence that repeated walkthrough reliability is
already accepted.

Routine final checks are recorded in:

- `target/offline-foundation-rust-final.log`
- `target/offline-foundation-clippy.log`
- `target/offline-foundation-js-final.log`

Current routine results: 324 Rust tests passed (four opt-in tests excluded),
122 JavaScript tests passed (desktop 24, agent 91, broker 7), build/types/lint,
Rust Clippy with warnings denied, and targeted source-format checks passed.
These counts are not a measure of full-product completion. The earlier 57-case
model score is historical: offline capability availability changed in this
package, and no new end-to-end model accuracy score is claimed.

### Development package

The release build and NSIS package completed. Evidence:
`target/offline-foundation-release-final.log`,
`target/offline-foundation-installer-final.log`, and
`target/offline-foundation-artifacts.json`.

- Output: `target/release/bundle/nsis/Custodian_0.1.0_x64-setup.exe`.
- Size: 1,209,896,244 bytes; unsigned development artifact.
- Installer SHA-256:
  `7223DCC76DFFB47A92A4E1155C2BFCE9F60AA7E514608AE62FB0B2D55941F29E`.
- Native executable SHA-256:
  `DDC09DEE87B33E2FAEE9635C65115486F3B785687916949104397F61008B40CD`.
- The outside-checkout live-test executable and agent module hashes match the
  packaged build and staged runtime. CPU model and dependencies are bundled.

WebView2 must already be installed. Tauri's automatic bootstrapper download was
disabled; the generated NSIS configuration excludes the download branch and
includes an explicit prerequisite refusal hook. The hook compiled successfully.
Missing-runtime, clean-profile install/uninstall and upgrade acceptance have
not been executed. In particular, the preinstall hook runs after Tauri's
reinstall-choice page; do not claim prerequisite refusal before every possible
upgrade/uninstall interaction. No missing-runtime registry state was simulated
on the user's machine. Generating this installer is not release acceptance.

### Real model smoke and a corrected weak assertion

The first updated-runtime smoke produced an inaccurate duplicate-file definition
(`copy ... with slight differences`) that the historical loose assertion accepted.
That result is retained in `target/offline-foundation-runtime-smoke.log`; it is
not evidence of correct knowledge. The smoke now requires content identity,
and the system prompt was corrected to match offline capability availability
and the application's byte-identical duplicate semantics.

The stricter rerun passed with the staged CPU runtime outside the checkout in
AppContainer, with GPU disabled and no unrestricted fallback: “A duplicate file
is an exact copy of another file, even if their names or locations differ.” It
also requested `check_disk_space` through the real protocol. The callback's disk
data is simulated in this isolated inference test; actual desktop file/drive
execution is established separately by the live UI test.

Evidence: `target/offline-foundation-runtime-final.log`, 51.28 seconds including
model verification/loading and both requests. This is one focused smoke, not a
general model-quality benchmark. General knowledge can still be wrong.

## Remaining gates

General handle-based path-race hardening and adversarial concurrent replacement
are not certified. App/screen/account private zones, scoped temporary grants
for arbitrary capabilities, complete data-retention invalidation, all legacy
route migration, dedicated Engine authority, unfamiliar multi-step planning,
local content/app editing, commands and clean-profile acceptance remain open.
The model sandbox does not network-isolate independently launched applications.
Do not describe this as universal privacy enforcement, instant interruption of
every OS call, a complete offline operator or a security guarantee.

Next implementation work is closing the remaining policy/execution boundary
gates before adding broader content and app actions. See the
[master plan](../plans/OFFLINE-HACKATHON-MASTER-PLAN.md) and
[planned acceptance matrix](../plans/OFFLINE-HACKATHON-ACCEPTANCE-MATRIX.md).
