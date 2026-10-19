# Model-planned file workflows — 17 October 2026

The shared installer/native output paths have since been replaced by the
[Stage 0 runtime repair](STAGE-0-RUNTIME-REPAIR-2026-10-17.md). Artifact hashes
below describe this earlier workflow build; use that report for current hashes.

This increment adds one shared execution engine for bounded file collections.
Qwen proposes typed capability sequences; native code validates their exact
selection semantics, binds paths and authority, prepares concrete file plans,
executes verified copies and saves group recovery records. It does not complete
the general desktop operator or establish a 10/10 product rating.

## Supported goals

Enable **automatic verified collections** in Routine permissions. The setting
is separate from single-file backup, defaults off, and remains off when an older
saved backup grant is loaded. Example requests:

- `Collect documents from "C:\Projects" to "D:\Backups"`
- `Please gather all my photos in "C:\Photos" into "D:\Collection"`
- `Could you copy the largest document from "C:\Projects" to "D:\Backups"`

The same capabilities support all documents, all images, and largest-file
selection with or without either filter. Sources can be arbitrary permitted
local folders or `"all drives"`; this is not limited to personal-folder
shortcuts. Both source and destination must already exist. The existing native
single-largest-file fast path is retained; the new collection requests use the
model planner.

Requests use a bounded vocabulary and quoted absolute locations. Unsupported
filters, mixed document/image goals, moving originals, archives and additional
instructions request clarification rather than copying a substitute set.
This is constrained composition, not arbitrary open-ended task planning.

Selection is based on filename extensions. It does not inspect document
contents, detect all secrets, deduplicate content, classify real file formats,
or promise that an output is safe to share publicly. Outputs use original
filenames in the existing destination; duplicate names and occupied outputs
stop preparation before the first copy. The complete selection must fit 20
files and 100 MiB; it is never silently truncated. The 5% destination free-space
budget and 1 GiB reserve are checked before copying and between steps.

## Model and native boundary

Only the person's native chat entry can initiate a collection. Native parsing
captures source, destination and a bounded selection contract. Qwen sees the
goal with `SOURCE`/`DESTINATION` slots and the authoritative native selection
contract, without actual paths or file contents. The grammar limits its
capability vocabulary to that contract; native validation remains necessary
because a model can still repeat, omit or misorder allowed capabilities.
Its dedicated authenticated IPC request has no desktop tools. It returns only
a JSON list chosen from discovery, document/image filtering, all/largest
selection, copy and verification capabilities. It cannot grant permission,
select a new location, run a shell or execute an operation while planning.

Native validation rejects unknown fields, unsupported capabilities, wrong
ordering and selection that differs from the actual request. JSON grammar is
syntax control; it is not treated as semantic validation. A failed model or
mismatched proposal does not fall back to automatic filesystem actions.
The agent can make one correction attempt using compiler feedback, then fails
closed if the second proposal is invalid. Attempt counts and rejected proposals
are retained; the native validator independently checks the final program.
Accepted programs compile to bounded streaming selection operators.
Normal chat history is restored after planning, including planning failure.

The original permission revision is captured before the native worker is
queued. Revocation invalidates planning and discovery. Each copy rechecks the
grant under its lock; a copy already running finishes safely, and later copies
stop. Privacy, credential, protected-location, ordinary-file and local-volume
checks remain in force. Laya is still experimental and has no production
authority. Windows handle guards reduce ordinary replacement/write races;
same-user malicious code, privileged attackers and writable mappings remain
outside the claimed guarantee.

## Durable truth and recovery

The workflow journal is DPAPI sealed. Every copy's receipt ID is saved in the
parent record before the file executor runs. This closes the identification
gap if the file receipt completes before the workflow publishes step success.
The file executor refuses reuse of that ID, even after undo; recovery never
replays changes automatically. Existing corrupt workflow history blocks new
workflow writes. Completion requires rechecking every source/copy pair against
the saved content and output identity.

Use **Show workflow status**, or ask `show workflow status`, to inspect the
latest saved workflow. Inspection rechecks receipts and actual files; a running
or incomplete saved record is reported as requiring recovery rather than
silently resumed. Status is available on demand, not a new startup dashboard.
Progress during execution is streamed through the existing task/chat UI.

Ask `undo workflow`, or use **Undo workflow**, for group undo of the latest
saved workflow. Only verified completed copies are removed. Originals,
changed outputs, unverified partial outputs and unrelated files are preserved.
If one output was edited, safe other copies can still be undone; the remaining
problem is reported explicitly. Inspection and undo also obey current privacy.
This does not implement retry/resume, archive creation or a general rollback
transaction across arbitrary applications.

## Evidence

Source, model, desktop and artifact results are recorded separately below.
They are separate checks; a correct proposal is not proof of task completion.

Final source checks passed: [342 Rust tests](../EVIDENCE/workflows/rust-tests.txt),
with four existing opt-in environment tests ignored; [all-target Clippy with
warnings denied](EVIDENCE/workflows/rust-clippy.txt); [131 JavaScript tests](../EVIDENCE/workflows/javascript-tests.txt);
[JavaScript lint](../EVIDENCE/workflows/javascript-lint.txt),
[TypeScript](../EVIDENCE/workflows/typescript.txt) and the
[production frontend build](../EVIDENCE/workflows/frontend-build.txt).

The [final actual CPU-model run](../EVIDENCE/workflows/final-constrained-planning.json)
passed 12/12 preserved regression cases, all on the first attempt. Median plan
time was 5,631.5 ms, maximum 14,785 ms, with 28,350 ms initial model warmup on
this machine. The model pin is included in the raw report. These are constrained
plan-assembly results with an authoritative native selection, not a new blind
language-understanding score. The bounded correction path is tested with
injected bad proposals; this particular real-model run needed no corrections.
The [raw model log](../EVIDENCE/workflows/final-constrained-planning.txt) and
[summary](../EVIDENCE/workflows/model-summary.json) are retained.
The [compiled agent module hashes](../EVIDENCE/workflows/agent-modules.json) matched
the staged installer runtime and the outside-checkout test runtime.

### Desktop integration findings

The first live workflow attempt failed at secure agent startup: its startup
probe timed out while native model libraries were being imported alongside
warmup. It copied no collection files and never fell back to an unrestricted
agent. The [failure is retained](../EVIDENCE/workflows/desktop-startup-failure.txt).
Isolated sandbox inference passed with both
[ordinary](../EVIDENCE/workflows/isolated-runtime-ordinary-path.txt) and
[verbatim Windows paths](../EVIDENCE/workflows/isolated-runtime-verbatim-path.txt).

The startup configuration check now resolves the runtime entry without loading
native libraries. A quick pong is not an inference-readiness certificate. Deep
checks and actual planning still await the initialized, integrity-checked model.
A regression test distinguishes quick configuration checks from initialization.
No sandbox, model pin or permission check was disabled to get the desktop run
working.

A subsequent run stopped while opening the collection-permission controls.
The [automation failure is retained](../EVIDENCE/workflows/desktop-controls-failure.txt).
The harness now sets and waits for the desired expanded/collapsed control state
rather than blindly toggling it. This is not a certification that every possible
single-click interaction is flawless.

The [first complete pipeline walkthrough](../EVIDENCE/workflows/desktop-final-03.txt)
passed all 22 checks with actual Windows controls, sandboxed Qwen inference,
fresh C:/D: fixtures and SHA-256 comparisons. It exercised document collection,
image collection, largest-document composition, existing-output refusal,
persisted permissions, the status/undo buttons after restart, partial group undo
around an edited output, unsupported-filter refusal, and revocation during
whole-PC discovery. Both user edits were preserved under distinct fixture names
by the harness after the app refused to remove them. No personal files were
mutation fixtures.

A repeat exposed another control-opening miss after restart; the
[repeat failure](../EVIDENCE/workflows/desktop-repeat-controls-failure.txt) is retained.
An accessibility diagnostic also exposed an unsupported legacy-pattern fallback
in the .NET automation client; that fallback was removed rather than counted as
a successful test. The [diagnostic failure](../EVIDENCE/workflows/accessibility-diagnostic-failure.txt)
is retained.

Inspection found a real product accessibility issue: the mascot exposed a
button role but handled pointer release and keyboard events without an
accessibility click handler. It now handles zero-click-count default-action
activation, cancelling any pending pointer gesture timer. Normal pointer drag
and double-click handling remain separate. Routine controls also restore focus
to their trigger after status/undo and associate their expanded panel by ID.
The harness uses supported invoke or expand/collapse actions, waits for actual
state and no longer falls back to global keyboard input. A final regression run
includes five mascot/control activation cycles before the file workflows.

The first post-accessibility run passed the five activation cycles but stopped
at a test restart because a captured browser child outlived its killed host.
The [cleanup failure](../EVIDENCE/workflows/desktop-child-cleanup-failure.txt) is
preserved. The harness now terminates that captured child handle if its exit
wait expires; it never searches for or kills unrelated browser processes.
The [first final walkthrough](../EVIDENCE/workflows/desktop-final-08.txt) passed
all 23 live checks after both startup and accessibility fixes. It includes five
activation cycles, approved move/restart/undo, standing backup permissions,
real-model document/image/largest-document workflows, selective group undo
around an edited copy, privacy persistence and whole-PC discovery revocation.
Its [control trace](../EVIDENCE/workflows/desktop-08/actions.txt), agent logs and
verified UI snapshots are preserved alongside the summary. The
[second fresh walkthrough](../EVIDENCE/workflows/desktop-final-09.txt) also passed
all 23 checks with the same executable and compiled agent modules. Its
[control trace](../EVIDENCE/workflows/desktop-09/actions.txt), logs and snapshots
are preserved. Both final processes exited successfully after test cleanup.

The next run reached collection planning but hit the unchanged 120-second
inactivity watchdog after CPU warmup. It reported failure and made no copies;
the [timeout evidence](../EVIDENCE/workflows/desktop-cold-planner-timeout.txt) is
retained. Inspection found eager evaluation of the ordinary chat system prompt
before the planner immediately replaced that history. Model loading now leaves
prompt evaluation to the actual request, avoiding that discarded CPU work.
The watchdog, cancellation rules and native authority checks are unchanged.
The existing 12-case model timings above describe the earlier eager-startup
snapshot, not a latency measurement of this startup change.

A sandbox test rerun could not reopen temporary transformed modules; its
[environment failure](../EVIDENCE/workflows/javascript-temp-access-failure.txt) is
preserved. Running with normal Windows temporary-file access passed all 131
JavaScript tests and lint after the lazy-prompt change.

The [development installer build](../EVIDENCE/workflows/installer-build.txt) passed.
The [artifact manifest](../EVIDENCE/workflows/artifacts.json) records the current
executable and installer hashes. The unsigned installer is 1,209,976,607 bytes,
includes the pinned offline runtime/model, and requires WebView2 already installed.
It has not been installed or subjected to clean-install/upgrade acceptance.
Bundling used the existing NSIS cache in the normal Windows profile after the
sandbox could not access that cache. No installer was executed or published.

The initial concurrent JavaScript run raced with the agent build deleting its
generated output. Its process test failed because the entry file was temporarily
missing. After the build completed, the rerun passed all 126 JavaScript tests.
The initial real-model evaluation was stopped after three failed development
cases under heavy resource contention; no held-out case had been reached.
A diagnostic reproduction preserved the wrong `select_largest` proposal for
document collection. The native validator rejects it without copying.
After dedicated planner instructions were introduced, all six development
cases passed. The frozen raw-language candidate then passed only 5/6 held-out
cases; `backup my images` incorrectly selected only the largest image. That
failure is preserved and is not reported as a passing language-understanding
benchmark.

The contract-bound candidate passed 11/12 reused regression cases. A stricter
dependency-wording experiment regressed to 3/12 and was rejected. Final checking
uses bounded compiler feedback with the native contract. Its results measure
constrained plan assembly, not unseen-language interpretation. All these
synthetic cases are author-written, not an independent external benchmark or a
general security certification. Expected answers were not changed.

Preserved evidence: [initial interrupted run](../EVIDENCE/workflows/initial-interrupted-run.txt),
[raw diagnostic](../EVIDENCE/workflows/raw-language-diagnostic.json),
[raw development](../EVIDENCE/workflows/raw-language-development.json),
[raw held-out result](../EVIDENCE/workflows/raw-language-heldout.json),
[contract candidate](../EVIDENCE/workflows/contract-candidate.json), and
[rejected prompt](../EVIDENCE/workflows/rejected-dependency-prompt.json).

The interruption regression uses real sealed disk journals with an interrupted
state and the completion-publication gap. It does not claim an actual process
was killed during copying. Clean installation/upgrade/uninstallation, abrupt
process-death acceptance, the original five-run offline walkthrough and broader
automation remain open.
Cross-process/multiple-instance permission coordination has not been certified;
the grant lock and execution serialization described above operate within one
assistant process. These checks do not establish all project security gates.
