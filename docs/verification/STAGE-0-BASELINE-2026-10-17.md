# Stage 0 — capability reconciliation and baseline

This records the initial audit and its failures. The subsequent
[runtime repair](STAGE-0-RUNTIME-REPAIR-2026-10-17.md) restores the tested baseline
and records the current Stage 0 acceptance result. Failures below are retained.

Only Stage 0 of the [ordered plan](../plans/OFFLINE-EXECUTION-PLAN-2026-10-17.md) is in
scope. This adds a register, frozen expectations, source preservation and audit
tooling. Application capabilities/authority were not expanded. No installer was
run, user settings changed, commit made or existing work reset.

## Coverage and findings

The canonical [register](../reference/CAPABILITY-REGISTER.json) tracks 58 requirements across
all original domains. Each has a component owner, status, source route,
dependencies, release gate, evidence references, acceptance task and expected
outcome. The [readable register](../reference/CAPABILITY-REGISTER.md) is generated from it.

| Disposition | Requirements |
| --- | ---: |
| Verified subset with boundaries | 8 |
| Implemented, current live acceptance absent | 4 |
| Partial broader requirement | 16 |
| Paused | 2 |
| Missing connected execution | 23 |
| Prototype | 1 |
| Explicitly deferred | 4 |

These counts are not completion percentages. Important source findings:

- Fixed enum app launches remain reachable; general program launch, app listing
  and closing are refused by the current privacy dispatcher. Older broad app
  claims do not establish current support.
- Six personal settings and persisted reminders have routes, but this stage
  does not certify their real desktop operation. They are implemented/unverified.
- The external UIAutomation harness is test infrastructure, not a production
  assistant capability for operating other apps.
- The reader explicitly returns unavailable; browser actions and orchestrator
  IPC throw unavailable. Earlier success-string placeholder descriptions are
  historical. The standalone parser decodes bytes as lossy UTF-8: no connected
  format-aware ingestion or containment is established by its presence.
- App observation/control, artifact editing, general heterogeneous planning,
  private apps/screens, cross-process authority and true mutation-time crash
  drills remain open. Sealed journal-gap simulation is not a process-kill drill.
- Laya failed semantic activation targets and remains experimental, without
  production authority. Qwen's constrained planner score is not general task NLU.

Online/accounts, later OS platforms and optional voice are explicitly deferred.
Broader settings/development/media/software capabilities remain on the offline
roadmap even beyond the currently authorized stages 0–6.

## Frozen expectations

Besides the per-requirement tasks, [15 representative specifications](../reference/STAGE-0-ACCEPTANCE-FIXTURES.json)
fix synthetic source texts, spreadsheet totals (100, 300, grand total 400),
report facts, archive members, private canaries and prohibited effects. They
cover filesystem, authority, apps, content, multi-app, installation and refusal.
They are future authored acceptance cases, not 15 executed tests, a command
whitelist or an independent holdout. Later stages must implement real harnesses
and vary wording, locations and state. Independent user evaluation remains open.

The [acceptance lock](../EVIDENCE/stage-0/acceptance-lock.json) detects expectation
changes. A pre-implementation review corrected the initial document collection
expectation: the existing filter includes CSV, so `data.csv` must be included.
Version 2 records that reason. The [initial specification](../EVIDENCE/stage-0/initial-fixture-specification.json)
and [initial lock](../EVIDENCE/stage-0/initial-acceptance-lock.json) are preserved.
No feature was changed or fixture execution failure observed to obtain this
correction. Subsequent changes require explicit reviewed versioning.

## Reference environment and target

[Measured environment](../EVIDENCE/stage-0/environment.json): Windows 11 Home Single
Language x64, 10.0.26300/build 26300; Ryzen 5 3550H, four cores/eight logical
processors; 14,968,041,472 bytes RAM; C: and D: NTFS local volumes. Installed GPUs
are inventoried, not newly qualified. CPU inference is the baseline.

Node is v24.18.0; the staged runtime records 133 packages and pinned Qwen 2.5
1.5B Q4_K_M. Notepad launcher 10.0.26100.8457 and Excel executable
16.0.20430.20146 exist. They are initial Stage 4 adapter candidates, not accepted
assistant adapters. Stage 4 must identify actual running app versions and prove
accessibility/API save behavior. No software installation is needed to start
those probes. WebView2 directories exist; clean-profile prerequisites remain open.

The accepted baseline is limited to the tested Windows x64 subset on this
machine. The release target is Windows 11 x64 ordinary-user operation; other
builds, filesystems, removable/cloud/reparse cases require separate acceptance.
There is no newly established broad hardware/OS support matrix.

Later format order: text/Markdown/CSV, then isolated DOCX/PDF/XLSX ingestion and
verified generation/editing, then presentations and declared image/media work.
Extension collection is not parsing, secret detection or safe-to-share analysis.

Proposed qualification targets, not new measurements: queued/progress indication
within one second; cancellation acknowledgement within three seconds; no later
step after Stop/revoke; safe-boundary completion for a commit already in flight;
and no indefinite model wait beyond the existing 120-second inactivity watchdog.
Long-task throughput/latency must be measured by workload and file-size class.

## Source preservation and repeatability

The [baseline manifest](../EVIDENCE/stage-0/baseline-final.json) records Git HEAD,
dirty-tree status, 313 selected source/config/resource files with hashes/sizes,
accepted native/installer hashes, compiled agent modules and the pinned model.
Git HEAD alone does not identify this dirty build. Existing changes were kept.

The local [source snapshot](../../target/stage-0/baseline-final-source-snapshot.json)
preserves exact selected bytes, lockfiles and source assets, approximately 2 MiB
before JSON/base64 overhead. [Verification](../EVIDENCE/stage-0/snapshot-verification.txt)
decoded every member, checked bytes/hashes and compared it with the checkout.
Restore only into a new directory, never over the active checkout.

The snapshot excludes dependencies, models, generated outputs, training data
and general docs. It is not a self-contained offline rebuild image. Rebuilding
needs the recorded toolchain and matching dependency/model assets. Bit-for-bit
compiler reproducibility is not certified. The native release was reused
unchanged; artifact identity does not establish clean-install acceptance.

From the repository root, using installed Node and normal Windows DPAPI/temp
access:

```text
node scripts/stage-zero.mjs
cargo test --workspace
npm test
node node_modules/eslint/bin/eslint.js scripts/stage-zero.mjs
```

`--render` regenerates the readable register; `--freeze` refuses an existing
lock; `--capture=<new-label>` creates a versioned manifest/snapshot and refuses
manifest overwrite. Hashing uses bounded streaming. The initial sandbox capture
was stopped after slow streaming and never published a complete baseline; final
capture used 4 MiB buffers and normal filesystem access. This audit-tool finding
is not a product-performance measurement.

## Validation and failures

- [Fresh Rust](../EVIDENCE/stage-0/rust-tests.txt): 342 passed, four ignored environment
  tests. Ignored cases are not passed cases.
- [Fresh JavaScript](../EVIDENCE/stage-0/javascript-tests.txt): 131 passed.
- [Audit-tool lint](../EVIDENCE/stage-0/audit-tool-lint.txt) passed after an unused
  destructuring variable was corrected. Product logic was not changed.
- [Register validation/capture](../EVIDENCE/stage-0/register-validation-final.txt)
  validates IDs, statuses, gates, dependency references/cycles, source/evidence
  links, rendered consistency and acceptance lock. Native/installer, agent and
  model hashes match accepted prior evidence.

The first fresh desktop attempt stopped after approved synthetic movement when
a test-owned child did not exit during shutdown. The [failure](../EVIDENCE/stage-0/desktop-first-run-failure.txt)
is retained and is not a completed walkthrough or proven product regression.
The harness previously captured lazy Process objects; waits resolved handles
only after parent termination. It now pins child handles beforehand, checks
start times against the captured process records and logs child identities and
exits, reducing exited/reused-PID ambiguity. No unrelated process is selected by
browser name for termination.

The [fresh repeat](../EVIDENCE/stage-0/desktop-second-run-failure.txt) failed at the
actual Qwen document collection after secure agent startup reported cancellation
and a ping timeout. The [UI state](../EVIDENCE/stage-0/desktop-run-02/failure-ui.txt),
[agent log](../EVIDENCE/stage-0/desktop-run-02/routine-restarted.stderr.log) and
[child-handle trace](../EVIDENCE/stage-0/desktop-run-02/actions.txt) are retained.
Captured children exited in this repeat, so the collection failure is distinct
from the first shutdown failure. No collection writes started, and no
unrestricted fallback was used. Historical 23-check successes remain historical;
they do not turn either of these fresh failures into a pass.

The [existing opt-in isolated-runtime test](../EVIDENCE/stage-0/isolated-runtime.txt)
also failed: its startup ping succeeded, the pinned model loaded in 62.2 seconds,
but the first real response hit the unchanged 120-second inactivity deadline
(120.17 seconds total test time). This is one additional opt-in failure, separate
from the 342 passing default tests and four default ignored cases. It shows the
problem extends beyond desktop shutdown; CPU/resource/runtime causes remain
unresolved. No root cause is certified from a timeout message alone.

The [overwrite guard check](../EVIDENCE/stage-0/freeze-overwrite-refusal.txt)
intentionally requested a second freeze; the tool rejected it without replacing
the lock. A subsequent register check passed. The changed harness also passed
PowerShell syntax parsing. These checks do not repair the failed runtime.

## Stage boundary

**Initial Stage 0 acceptance was open.** The register, frozen specifications and source
preservation are complete; the fresh desktop baseline is not accepted because
of the startup failure. Do not advance to Stage 1 on the unit-test counts alone.
The immediate prerequisite is to diagnose secure startup/desktop lifecycle and
rerun the same baseline without weakening isolation or increasing deadlines to
hide the failure.

Stage 0 requires a complete register, frozen specifications, preserved source
and runtime identities, measured prerequisites and a working existing baseline.
It does not close missing product/security/installation requirements. Stage 1
is a separate next increment: shared authority, private observations,
cross-process coordination and real crash recovery. No Stage 1 feature has been
implemented by this audit.
