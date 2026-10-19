# Build repair execution record

The 19 October audit remains historical evidence. This assignment implements
repairs and the remaining build plan in dependency order; it does not relabel
the audited candidate as accepted. Existing user changes and frozen acceptance
expectations are preserved.

## Current work

Stage 4 editor shutdown is the first gate. The audited candidate passed three
editor tasks, then failed its fourth because the extension exited while the
application remained alive beyond the existing deadline. The application later
exited. Isolated shared storage was already in use. Diagnose normal shutdown
before changing behavior. Trace-enabled runs are diagnostic, not acceptance.

## Required sequence

Resolve editor lifecycle and rerun unchanged interference, privacy, cancellation,
recovery and repeated application gates. Repair model correctness using varied
frozen quality cases and actual local inference. Qualify the installed Excel
patch before changing the supported-version boundary. Then connect Stage 5
file/storage primitives through native manifests, authority, receipts and UI;
qualify native recycle/restore. Deliver contained Stage 6 content workers,
document/media formats and independently verified multi-app workflows. Finish
matching optimized packaging and actual disposable offline installation checks.

## Completion rules

Keep every failed attempt and exact scope visible. A passing build, mocked
adapter or isolated core API does not establish a completed product route.
Do not weaken deadlines, private-zone rules, normal-close requirements or
source preservation to turn a failure green. Installation remains unaccepted
until an actual disposable Windows environment is identified and exercised.

Working logs: `target/build-repair-2026-10-19/`. Trace diagnostics are complete
and their temporary flags have been removed. Current work qualifies native
application safety and model quality before a new optimized release candidate.

## Editor deadline correction

Eight native trace runs and five desktop trace runs passed. They did not
reproduce the previous intermittent shutdown failure, so they are not evidence
of its exact cause or a production fix. The desktop diagnostic then correctly
refused the installed unqualified Excel version.

Code inspection found a separate deadline inconsistency: the Close request
declares 30 seconds, but the process-exit portion aborts after 15 seconds.
The exit wait now delegates to the existing action deadline and Stop/authority
checks, without a second, earlier timeout. The external 30-second budget is
unchanged. Both application and extension process exits are still required;
no process is killed to report successful completion. Temporary trace arguments
were removed.

Two real-process regression tests pass: a hidden test-only helper exits
naturally after 17 seconds within the 30-second budget, and deadline/revocation
refusal returns promptly while the helper remains alive. Only the test-owned
helper is terminated during test cleanup. Untraced application and optimized
acceptance remain pending.

## Model quality fixture

Eight developer-authored factual cases, repeated twice, were frozen before
comparison. One is the exposed audit failure; this is a regression set, not
independent holdout evidence. The evaluator records answers, tool attempts,
timeouts, model/module hashes and every failure. Baseline uses the unchanged
audited runtime; candidate behavior has not yet been accepted.

The unchanged baseline finished with 14/16 automated passes. Semantic inspection also
finds misleading affirmations in Q03/1 and Q06/2 despite their matching a regex;
automated scores alone do not establish factual correctness. The candidate
changes only general free-form decoding temperature from 0.4 to 0, matching the
existing deterministic tool/planning policy. No factual-case prompt or frozen
assertion was changed. Candidate comparison and semantic review remain pending.

## Excel patch qualification candidate

Admission and the COM worker now share one exact-version check, listing the
historically tested 16.0.20430.20146 and installed 16.0.20430.20182. This source
change is a qualification candidate, not acceptance of the new patch. Adjacent
patches and other Office versions remain refused. Native creation/edit/save,
independent reopen/calculation, unrelated-workbook preservation and optimized
desktop acceptance must pass before shipping the candidate. The installed
Office application itself is not modified or downgraded.

The rebuilt native Excel workflow passed on 16.0.20430.20182: eight coordinated
steps, original preservation, recalculation, out-of-scope cell preservation,
real reopen, publication, sealed restart evidence and safe undo. A separate
unsaved synthetic workbook was preserved and closed normally. This establishes
native candidate behavior; optimized desktop and disposable installation
acceptance remain pending. See `excel-native-01.txt` in the working logs.

The rebuilt editor workflow also passed without diagnostic trace flags:
eight steps, original preservation, publication, restart evidence and safe undo
(`editor-untraced-01.txt`). Repeated optimized and safety gates remain separate.

Seven untraced real-editor adversarial scenarios subsequently passed: stale
state, private tab, revocation before execution, revocation after save, Stop,
forged targets and actual process-crash recovery. Recovery was inspected twice
without replay, later edits were preserved, and normal fixture exit was proved.
Duplicate-window interference and native-modal checks subsequently passed with
unrelated unsaved canary work preserved and normal cleanup proved, bringing
this native adversarial rerun to nine passing scenarios. The optimized desktop
repetitions remain pending. The exact Excel version
boundary unit test also passes, including refusal of adjacent unqualified
patches. Completed native logs are retained in
`docs/EVIDENCE/build-repair-2026-10-19/`.

## Disposable installation environment selection

The user delegated selection of the disposable environment. Select a full
disposable Windows VM with a snapshot and the qualified Code/Excel versions,
so that restart and Office checks can run in the same environment. Do not use
the normal host as a substitute or disconnect its network.

Read-only discovery on 19 October found Windows edition `CoreSingleLanguage`
(Home Single Language), build 26300, and no WindowsSandbox executable or Get-VM
command. The ProductName registry string is legacy and reports Windows 10;
the edition and build are recorded rather than inferring an OS marketing name.
Processor CIM discovery was denied in the restricted runner. No VM was started,
created or certified by this discovery.

Microsoft explicitly excludes Home from supported Windows Sandbox editions:
https://learn.microsoft.com/windows/security/threat-protection/windows-sandbox/windows-sandbox-overview

A full VM is selected, but no available hypervisor, guest image, connection or
Office-equipped disposable machine has been discovered. Selection does not
establish access. Actual installation acceptance is pending that environment;
host feature installation, licensing and restart changes have not been made.
The guest setup, offline isolation, installer refusal, ordinary-user execution,
restart, recovery and uninstall procedure is recorded in
`docs/verification/OFFLINE-VM-ACCEPTANCE-2026-10-19.md`. It is a prepared procedure, not a
provisioned VM or passed installation test.

## First candidate quality result

The temperature-zero candidate completed with 15/16 automated passes, improving
on the baseline's 14/16, but still falsely guaranteed restoration from compression
in Q06/1. Q04/1 also included unsupported format/size advice beyond its correct
primary answer. The quality gate fails; this is not an accepted quality repair.
Recorded repetitions also differ despite temperature zero, so reproducibility
is not guaranteed by that setting. Preserve `chat-quality-candidate-01.json`
and its log alongside the baseline; no frozen expectations were relaxed.

A second candidate adds general factual calibration instructions: separate
effects from proof, qualify conditions, acknowledge uncertainty and avoid
unrelated claims. It contains no case-specific backup definition or canned
reply. It is evaluated against the same frozen cases and deadline. Its results
must be reviewed semantically even where regular expressions pass.

The second 1.5B candidate also scored 15/16 automatically and failed the gate.
Semantic inspection additionally rejects Q06/2, whose false restoration
guarantee passed a regex because an unrelated sentence included "not"; Q03/1
also contradicts its premise while starting with the correct conclusion.
This inspection is by the coding agent, not an independent human evaluator.
The 104 agent unit tests and focused ESLint checks pass, but they do not establish
factual quality. A diagnostic of the larger already-pinned 4B model follows on
the same exposed failure, with the same per-case deadline. A partial diagnostic
cannot pass the complete quality gate, even if its selected cases pass. No
runtime model selection or package manifest has been changed.

The 4B CPU diagnostic also fails: its first Q06 trial reaches the unchanged
90-second deadline without an answer; the second completes in 37.35 seconds.
Although that second answer matches the regex, it incorrectly says compression
does not preserve folder structure, so semantic inspection rejects it. This is
only two trials of one exposed case, not a complete-suite result. No larger
model is accepted or selected for the release. Local GGUF metadata inspection
confirms the two files identify qwen2 and qwen3 architectures respectively;
their filenames alone were not used to infer architecture. Raw results and
metadata are preserved with the repair evidence.

## Status at this evidence checkpoint

Native editor and Excel candidate workflows and nine editor safety scenarios
pass. The focused native regression tests (two close-budget tests and one
version-boundary test), 104 agent unit tests, focused ESLint, source formatting
and tracked diff whitespace checks pass. No fresh full-workspace test or
optimized release acceptance is claimed for these source changes.

Do not promote the candidate: factual quality is still unresolved, repeated
optimized desktop editor/Excel gates remain pending, and no disposable guest is
available for offline installer acceptance. Stage 5 integration and Stage 6
format/worker gaps from the audit also remain open. This checkpoint records
progress, not completed stages or a perfect build.
