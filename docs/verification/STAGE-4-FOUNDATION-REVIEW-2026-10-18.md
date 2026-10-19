# Stage 4 foundation: user-work verification and native admission

18 October 2026. Status: native foundation increment verified, including the
Windows desktop development build with embedded frontend assets.
Stage 4 as a whole is not accepted. No real editor/spreadsheet adapter is enabled.

## What was reviewed

The user's checkout at `b49953e` contains the accepted Stage 3 work plus local
Stage 4 additions: typed app requests, an executor trait, a text-editor program
compiler, privacy filters, process-generation measurement, mock coordinator
tests, and the app-adapter design contract. Dockerfile/Makefile/.dockerignore and
a missing Temporal workflow module were also added locally.

The original 13 reviewed modified/untracked files were preserved before edits
in `target/stage-4/user-work-before-review.json`. The user changes were built
and tested before extending them: **404 native tests passed, 5 ignored**;
TypeScript checking and **138 JavaScript tests passed**. The first restricted
JavaScript run encountered Vitest temporary-cache ENOENT failures; running under
the normal Windows profile passed without changing application code for that
environment problem.

The app tests were mock/contract tests, not evidence that Notepad or Excel had
been controlled. The default privacy filter treated a valid non-private identity
as eligible, but the execution helper had no independent positive app grant.
The timeout field was only validated, not enforced by a worker. Edit requests
lacked an exact range and normal multiline text was rejected as control data.
The capability check could be bypassed by tagging an app key with a different
step kind. Post-effect cancellation also discarded a returned receipt.

These were problems in an unconnected foundation, not demonstrated exploitation
of an enabled general desktop operator. Live app observation remains disabled.

## Implemented changes

- Added `app_authority.rs`: native, non-deserializable, expiring and revocable
  grants. Scope includes issuing authority, full app identity, native document
  ID, document version and allowed effect classes. Edit authority additionally
  binds the exact range and replacement digest. A grant cannot be transferred
  to a different authority instance or revived by cloning after revocation.
- The authority stores a current privacy revision behind a native lock. Stale
  settings updates fail; replacing policy invalidates older grants. This is an
  in-memory library, not a persisted settings/UI implementation.
- Replaced the misleading `execute_bounded` helper with `execute_checked`.
  It checks authority before metadata measurement, requires native remeasurement,
  rechecks authority and exact target/document/version, then validates returned
  receipt identity/evidence and current authority after execution. An adapter
  with no remeasurement implementation defaults to unavailable.
- Introduced a distinct opaque document ID in requests/receipts. Duplicate
  display names and equal document contents do not establish the same document.
- Added cooperative Stop/deadline checks for adapter calls. Timeout bounds are
  validated before constructing a deadline, including adversarial `u64::MAX`.
  This cannot preempt an unresponsive native call; the hard-timeout worker is
  still missing and remains mandatory before qualifying real app control.
- Retained returned effect receipts on late Stop, revocation, mismatched or
  unverified outcomes. Error Debug output omits receipt/document content. This
  evidence is not a durable write-ahead journal or permission to replay/undo.
- Enforced registered app capability/kind pairs for all `app.*` steps. Inspect
  is Read; open/edit/save/close are App; output verification is Verify. Changing
  a step kind cannot disguise a registered edit as a read.
- Added explicit bounded edit ranges, multiline/Unicode support, rejection of
  unsafe control characters, strict request schemas, conservative local
  executable spellings, stale snapshot revision checks and valid UTF-8 excerpt
  truncation. None of these shape checks constitutes native identity proof.
- Extended the user's process-generation work with a metadata-only probe that
  measures executable path and creation time through the same open process
  handle. The native test probes its own process only; it does not enumerate
  another application's titles or content.

Existing suitably scoped standing consent may cover multiple native step grants;
these bindings do not require a new prompt for every ordinary action. The host
must independently bind paths, targets and policy before issuing them. Qwen
cannot deserialize or manufacture a grant.

## Environment and future acceptance fixtures

`scripts/probe-stage-four-apps.ps1` records only fixed installed-executable and
package version metadata, creates new evidence without replacing old evidence,
and never launches an application or reads window/document content.

The measured candidates are:

| Candidate | Measured version | What this proves |
| --- | --- | --- |
| Windows Notepad launcher | 10.0.26100.8457 | Launcher file exists; not the actual editor version |
| Packaged Notepad editor | 11.2607.14.0 | Actual editor executable metadata exists |
| Excel | 16.0.20430.20146 | Executable metadata exists |

None is a qualified adapter. The probe launched zero apps and performed zero
window-title or document-content observations.

[Twelve future real-app cases](../EVIDENCE/stage-4/app-acceptance-fixtures.json)
are recorded before worker/editor implementation. They cover duplicate titles,
multiline edits, private windows and tab switches, focus/movement, stale native
identities, dialogs/macros/links, hangs/Stop, revocation, crash gaps, unrelated
workbooks and optimized offline execution. They are definitions, not passing
results or independent evaluation. Existing Stage 0 expectations are unchanged.

The new authority regression cases were authored with this review/fix; they
are not an independently withheld benchmark. Some execute a real temporary
file effect to verify evidence retention. They do not control a real editor.

## Build tooling review and limits

The new Temporal workflow module compiles during TypeScript checking and resolves
the missing source-module problem. Its IPC activity still throws unavailable;
no Temporal-server execution or supervisor integration is claimed.

Docker and GNU Make are not installed/discoverable in this environment, so no
image build, `make` invocation or Linux/container acceptance is claimed. Relevant
test commands were run individually. Python tests pass on the available Python
3.11.9; `.python-version` and the proposed Docker image target 3.12, which was
not qualified here. No dependency or software installation was attempted.

The native release file still has the accepted Stage 3 SHA-256
`1841cb92fc7d0c7e7948d3572b858e1595097003a84be5f91903c523b79889ad`.
It must not be relabelled as Stage 4. The foundation development build, final
test counts and selected-source identities are recorded separately in
[result.json](../EVIDENCE/stage-4/result.json). No new optimized Stage 4 desktop,
installer execution, signing or clean-install acceptance is claimed.

## Remaining work and next implementation gate

This increment does not complete 4.0/4.1 qualification. Installed metadata and
core contracts are verified subsets. Persistent private-app/document policy,
host UI/grant issuance, native file/privacy binding of document IDs and mixed-tab
privacy must be implemented and accepted before content observation is enabled.

Next build the supervised app worker: authenticated bounded messages, typed
operations only, job/resource limits, hard call/task deadlines, cancellation,
target/document remeasurement, bounded observations and write-ahead effect
checkpoints. Prove containment and privacy first with synthetic owned windows.
Do not grant arbitrary PowerShell, global keystrokes, screenshots or shell calls.
Do not force-close an unrelated application to stop a worker.

Only then qualify real editor open/edit/save/reopen/normal-close behavior,
preserve unrelated unsaved work, and connect the accepted adapter to the
coordinator and planner. Qualify Excel separately, including macro/link behavior,
explicit workbook/range bindings and independent saved-value verification.
Follow the unchanged [detailed stage plan](../plans/STAGES-4-5-6-DETAILED-WORK-PLAN-2026-10-18.md).
Stages 5/6 have not started; Laya remains inactive. Full-product acceptance is open.

## Evidence

Final checks: **418 native tests passed, 5 opt-in tests ignored**; **138
JavaScript tests** and **70 Python tests** passed. TypeScript checking, strict
workspace Clippy, frontend lint/build and the secret-pattern scan passed. The
development desktop was built with
`cargo build -p custodian-desktop --locked --features tauri/custom-protocol`.
These results accept the foundation increment only. No new real-app walkthrough
or optimized Stage 4 release was run.

The final selected-source delta and lockfiles are captured and restored into a
fresh directory, with a separate backup and exact hashes in
[result.json](../EVIDENCE/stage-4/result.json). Reconstruction also needs the named
Git base, dependencies and runtime assets; this is not a self-contained release
or certified reproducible build. Existing Stage 3 release/source evidence stays
unchanged. The original user Docker/Makefile/workflow additions are preserved.

- [User baseline native tests](../EVIDENCE/stage-4/user-baseline-rust-tests.txt)
- [Normal-profile JavaScript tests](../EVIDENCE/stage-4/user-baseline-javascript-tests.txt)
- [Final native regression](../EVIDENCE/stage-4/rust-tests-accepted-foundation.txt)
- [Python tests](../EVIDENCE/stage-4/python-tests.txt)
- [Strict Clippy](../EVIDENCE/stage-4/clippy.txt), [lint](../EVIDENCE/stage-4/lint.txt),
  [frontend build](../EVIDENCE/stage-4/frontend-build.txt)
- [Metadata-only app inventory](../EVIDENCE/stage-4/app-environment.json)
- [Secret-pattern scan](../EVIDENCE/stage-4/secret-scan.txt)
- [Development desktop build](../EVIDENCE/stage-4/desktop-development-build.txt)
- [TypeScript check](../EVIDENCE/stage-4/typecheck.txt)
