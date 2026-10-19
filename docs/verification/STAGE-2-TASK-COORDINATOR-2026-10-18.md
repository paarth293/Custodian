# Stage 2: bounded task coordination

Status: Stage 2 accepted on the reference Windows machine. Native/frontend
checks and both 28-check desktop runs passed, including the optimized executable
and successful fixture-process cleanup.
This increment implements the coordinator foundation. It does not enable real
app control, general language planning, content editing, scripts or online work.

## Execution and evidence

`custodian-core::task_program` executes a bounded native program with a goal,
constraints, descriptive scoped grants, typed steps, explicit dependencies,
observed facts, receipt references, branch predicates and completion tests.
Discovery, read, write, app and verify are separate step kinds. Fact references
must point to an earlier declared dependency. Programs contain at most 64 steps;
observation retries allow at most two extra attempts. There are no loops or raw
execution commands. A completion predicate must reference a verifier step.

Grant descriptions are evidence, never consent. Native adapters must recognize
the capability and its kind, recheck real permission and enforce commit-time
policy. The production collection adapter accepts only preflight, collection
copy and collection verification. Its existing copy-only permission, captured
identities, hashes, privacy checks, capacity budgets and sealed write-ahead
receipts remain mandatory. An unknown capability fails before adapter execution.

The coordinator checkpoints the in-flight step before execution and records its
facts and receipts afterwards. Writes and app effects must return receipt
references; an absent receipt requires recovery. Their execution is never retried
automatically. Recorded effects with failed final verification produce a partial
result; an uncertain in-flight effect requires recovery. Neither claims success.
Restart retains evidence, marks active work for review and never replays it.

Failures distinguish retryable observations, stale world state, missing
authority, unavailable capability and unrecoverable failure. Only explicitly
retryable observations get the bounded retry, with fresh adapter authorization
on each attempt. No automatic replanning or replacement of a running program is
enabled. A new user request rediscovers facts; model-driven repair is a later
stage and must not infer permission from a failure message.

## Production integration and controls

Real collection requests now use the shared coordinator for three native steps:
preflight of captured files, the existing receipt-backed collection executor,
and an independent recheck of every copy. Discovered input and output paths are
reserved atomically before effects, in addition to the destination lock held
during planning. Parent/child conflicts refuse the whole additional reservation.
Locks remain held while a worker is running or paused and release only after its
terminal state has been saved.

Pause is cooperative at a program-step boundary. For this adapter a whole bounded
collection is one write step, so pause can wait for that collection to finish.
Stop also retains the existing between-copy checks; an already committing copy
finishes safely. Pause does not interrupt a file write or promise instant
suspension. Restart cannot resume an old pause. The coordinator rechecks task
Stop and live request/privacy authority at boundaries and while paused.

The task view displays step progress, recorded effects, receipt counts, failure
classification and pause acknowledgment. It retains the ten most recent
terminal program tasks, plus active tasks and tasks requiring recovery. The
latest-collection inspection control is explicitly labelled as selecting the
latest saved collection; it does not imply arbitrary historical group undo.
Private task resources and serialized execution evidence are filtered before
projection. History describes completion at execution time, not a claim that
files can never change later; inspect the saved collection for current status.

Starting a new conversation invalidates queued authority, stops old work and
clears pending cards/plans before waiting for the model lock. Old request
bindings cannot start a later step or add stale facts back into chat context.

## Verification boundary

- Native workspace: **379 passed**, **4 environment-dependent tests ignored**.
- JavaScript workspaces: **134 passed**, including task-view evidence rendering.
- Strict native static checks, frontend lint and frontend build passed.
- The embedded-assets development desktop passed all **28** live checks and
  its acceptance driver exited successfully after fixture-process cleanup.
  This includes the task-view evidence/recovery control, real Qwen collections,
  C:-to-D: operations, private canaries, scoped permissions, revocation, restart,
  selective undo and whole-system reads with an independent chat request.
- The optimized production-protocol executable independently passed the same
  **28** checks and its acceptance driver exited with code 0 after cleanup.
- Deterministic fixtures exercise all five step kinds, dependency-fed inputs,
  branching, observation retry limits, unavailable capabilities, descriptive
  grant forgery refusal, uncertain writes, missing effect receipts, unmet final
  tests, resource conflicts, pause/resume, individual Stop, stale conversation
  epochs and encrypted journal reopening without replay.
- Real file fixtures execute collections through the coordinator, independently
  verify receipts, preserve originals, refuse stale inputs and undo verified
  copies. A separate fixture composes actual file effects with an explicitly
  **mock application acknowledgment**, through the same runner. It controls no
  real application and is not app automation acceptance.

Evidence lives in `docs/EVIDENCE/stage-2`. Initial sandbox runs encountered DPAPI
access denial and missing JavaScript transform-cache files in the sandbox temp
directory. Normal-profile runs passed; these environment failures are not
reported as successful executions. Earlier passing intermediate runs are
retained separately from the final source checks.

## Accepted artifact

The tested executable is
[Custodian-stage2-production.exe](C:/Users/paagu/.codex/visualizations/2026/10/16/01a142c7-c123-7122-873a-8f2707fc4063/Custodian-stage2-production.exe).
Its SHA-256 is
`BF2144648D70A586AD5262181D1BCA1DCFB46816B513BF3422D6DBC853E8E204`.
It matches `target/release/custodian-desktop.exe`, built with
`cargo build -p custodian-desktop --release --features tauri/custom-protocol`.
The optimized build took 7 minutes 31 seconds. No source changes were made to
the validated application during optimization.

This is an unsigned development artifact, not an accepted installed release.
The native executable is 6,341,120 bytes. Static checks confirm x64, ASLR and
DEP; Guard CF is absent. Test-only crash/lock environment hooks are absent in
both UTF-8 and UTF-16 encodings.
Compiler hardening and installer acceptance remain separate gates. Exact
artifact metadata, executable checks, selected-source preservation and restore
verification are recorded alongside the test logs in the Stage 2 evidence.

The accepted selected-source snapshot contains 321 files / 2,190,610 bytes.
Each file was reconstructed into `target/stage-2/restored-accepted` and checked
against both its stored hash and the current source. A second snapshot copy is
preserved outside `target` as `Custodian-stage2-source-snapshot.json` in the
artifact directory. Snapshot SHA-256:
`300681ba0e45eebe85eed5932713996082fdf229902bcc2b14b858f4d2b6bcd2`.
The first helper attempt refused an existing Stage 1 backup filename; the guard
prevented replacement. The corrected Stage 2 copy and original Stage 1 backup
were independently verified. These snapshots exclude dependencies, models,
generated artifacts and general documents; they are not self-contained builds.

## Remaining scope

Stage 3 language extraction/planning and Laya evaluation have not started.
Actual app observation/control remains Stage 4. Most existing scan, setting,
single-file and reminder routes still use their established native lifecycle;
they are not all converted into composed programs. The alternate TypeScript
orchestrator remains unavailable. Broad content/development/software-management
scope, full security acceptance, signing and clean installation remain open.
The historical Stage 0 installer does not contain this increment.
