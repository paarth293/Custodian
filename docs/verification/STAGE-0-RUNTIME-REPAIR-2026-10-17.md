# Stage 0 runtime baseline repair

Started 17 October; completed 18 October 2026 (Asia/Calcutta).

This is a continuation of Stage 0, not Stage 1 or a capability expansion.
The original [failed audit](STAGE-0-BASELINE-2026-10-17.md) and its raw results
remain evidence. No original acceptance task, deadline, model pin or expected
answer was relaxed.

## Diagnosis and change

The agent's Windows job explicitly forced below-normal process priority.
On this reference machine that configuration incurred large delays before
inference: the unchanged isolated AppContainer run spent 24.1 seconds hashing
the model and never completed the first response within the inactivity limit.
Its worker had consumed only 2.375 CPU seconds at roughly 100 seconds elapsed,
so the observed delay was not simply a slow stream of generated tokens.

An existing diagnostic without AppContainer, but with the same below-normal
job, also timed out. A diagnostic-only ordinary-priority process using the same
pinned CPU model completed in 39.38 seconds. That diagnostic had no desktop
tools and is not a production fallback or an accepted desktop workflow.

The candidate changes one production setting: the agent job no longer forces
below-normal scheduling. The 4 GiB job memory cap, kill-on-job-close behavior,
AppContainer isolation, authenticated IPC, privacy policy, approval rules and
30-second startup/120-second inactivity/3-second cancel deadlines are unchanged.
Inference runtime concurrency remains its existing bounded configuration.

The same existing isolated test then passed inside AppContainer: hashing 2.4
seconds, native runtime import 5.0 seconds, model ready at 11.3 seconds, and
71.01 seconds total for real chat plus a validated tool request. A process
sample confirmed Normal priority. This isolates scheduling as a material cause
on the measured machine; it is not a claim to have identified every Windows
kernel/storage mechanism or qualified arbitrary hardware.

## Preserved comparisons

| Run | Isolation / scheduling | Result |
| --- | --- | --- |
| [Unchanged reproduction](../EVIDENCE/stage-0-repair/unchanged-runtime-reproduction.txt) | AppContainer / below normal | Failed: original inactivity watchdog; 139.63 seconds total, including startup |
| [Plain job control](../EVIDENCE/stage-0-repair/plain-job-control.txt) | Diagnostic without container / below normal job | Failed: original inactivity watchdog; 128.88 seconds total |
| [Ordinary-priority diagnostic](../EVIDENCE/stage-0-repair/normal-priority-control.json) | Diagnostic without desktop tools or job | Passed: 39,384.9 ms, correct real answer |
| [Sandbox candidate](../EVIDENCE/stage-0-repair/ordinary-job-sandbox-candidate.txt) | AppContainer / ordinary scheduling / memory job | Passed: 71.01 seconds, real answer and tool-request check |

These are single-machine diagnostic comparisons, not independent benchmark
scores or a broad performance guarantee. The normal-priority diagnostic started
near the end of the preceding failed plain-job control; both failed controls
had already accumulated long delays. Final acceptance must use the sandboxed
production executable, not this diagnostic script.

The new diagnostic is `services/agent/eval/runtime-startup.mjs`. The audit tool
also accepts `--artifacts=<repository-relative manifest>` so a repair can be
captured without overwriting the earlier workflow artifact evidence. Existing
acceptance-lock and baseline-overwrite protections remain in force.

## Validation

Fresh JavaScript tests: 131 passed. JavaScript lint and diagnostic/audit format
checks passed. The native default suite passed 342 tests (four ignored), and
all-target Clippy passed with warnings denied. The accepted production build
uses the command below and SHA-256
`692B1A2681F0FAF7EFBB7F9312DE9B19BEE9C6CD6BB5C49C2296D262BAA12C05`.
See the raw files in
[repair evidence](../EVIDENCE/stage-0-repair).

The first release command omitted Tauri's production protocol feature. The
resulting executable opened a localhost development URL and failed before app
checks. This is a build-command error, not a passing app run or evidence against
the isolated scheduling result. The [rejected build record](../EVIDENCE/stage-0-repair/rejected-development-build.txt)
and [observed UI](../EVIDENCE/stage-0-repair/rejected-development-ui.txt) are retained.
Its SHA-256 is explicitly rejected for delivery. The corrected production command
is:

```text
cargo build -p custodian-desktop --release --features tauri/custom-protocol
```

The production feature embeds the existing frontend instead of requiring a
development server. It changes neither the acceptance cases nor model isolation.

Both fresh production walkthroughs passed all 23 unchanged checks and exited
successfully: [run 02](../EVIDENCE/stage-0-repair/desktop-final-02.txt) and
[run 03](../EVIDENCE/stage-0-repair/desktop-final-03.txt). They exercise repeated
accessibility activation, private canary exclusion, real cross-drive approved
move/restart/undo, separate standing grants, real Qwen document/image/largest
document workflows, occupied-output refusal, selective group undo preserving
user edits, unsupported-filter clarification, whole-PC discovery cancellation
and revocation, and native offline handoff refusal.

The [run 02 trace](../EVIDENCE/stage-0-repair/desktop-02/actions.txt) and
[run 03 trace](../EVIDENCE/stage-0-repair/desktop-03/actions.txt), per-launch model
logs and verified UI snapshots are retained. All mutations used synthetic
fixtures. No acceptance timeouts were increased and no expected answer changed.
The fifteen frozen future fixture specifications are not counted as these 23
executed checks or falsely marked all implemented.

The model, compiled agent modules, runtime dependencies and application feature
scope are unchanged. The shared installer output now contains the accepted
repair. The [artifact manifest](../EVIDENCE/stage-0-repair/artifacts.json) records
the exact native and installer hashes. The installer is 1,209,976,627 bytes,
SHA-256 `7FFA76E2FB1BF510779E9DF7E7F0B16495596AB8DE4BD7B3472B0C599F7D9309`.
It is an unsigned development artifact requiring existing WebView2. It was
packaged, not installed; clean install, upgrade and uninstall remain unaccepted.
The [earlier installer/native identity](../EVIDENCE/stage-0-repair/preserved-previous-artifacts.json)
is preserved separately rather than silently overwritten in historical evidence.

The [repaired source/runtime baseline](../EVIDENCE/stage-0/baseline-repaired.json)
records a new selected-source snapshot and explicitly references the repair
artifact manifest. The original baseline and its source snapshot remain intact.
The same 58 requirements and 15 frozen fixture specifications are retained;
the acceptance-lock digest is unchanged. Source preservation does not certify
bit-for-bit compiler reproducibility or a self-contained offline rebuild image.
The [final snapshot verification](../EVIDENCE/stage-0-repair/final-snapshot-verification.txt)
decoded and hashed every selected source member and compared it with the current
checkout. [Before/after backups](../EVIDENCE/stage-0-repair/source-backups.json)
also preserve both source snapshots outside the build-output directory, so a
later build cleanup does not remove the only copy.

## Stage exit

**Stage 0 is complete for the recorded reference baseline.** The
[final result](../EVIDENCE/stage-0-repair/result.json) records passing checks,
artifact/source identity and unchanged frozen expectations. This closes the audit
and existing-baseline gate, not all requirements in the capability register.
Stage 1 has not started. Broader authority, app control, content tools, crash
drills, clean installation and independent user evaluation remain their separate
later gates; these successes do not establish whole-product acceptance.
