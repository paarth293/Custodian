# Stage 3: grounded language and file planning

Status: the declared file-language increment is accepted. Frozen evaluation,
native execution, runtime checks and the optimized desktop walkthrough passed.
This is the language/planning increment for
the currently registered copy operators, not general app or content automation.

## What changed

Previously the desktop recognized a narrow sentence pattern, selected a filter
and selector itself, then supplied that authoritative selection to Qwen. The
model mainly assembled an already specified sequence. That regression score
was not evidence that the model understood a new goal.

The new route measures two explicitly quoted existing local folders, or the
explicit all-drives source scope. It replaces their path text with location
IDs before asking Qwen to interpret the request. Qwen chooses the semantic
operator combination and source/destination roles. The agent expands that choice
into a fixed registered logical chain; native code independently validates its
exact dependencies and mandatory copy verification before building the executable
workflow. The model does not repeat deterministic dependency bookkeeping. New wording and
reversed location order use the same route, without new sentence handlers.

The current semantic combinations are all files/documents/images, optionally
selecting the largest. Document/image selection remains extension-based, not
content understanding. Logical operators are discovery, optional category
filtering, all/largest selection, copy and verification. Native execution still
uses the Stage 2 coordinator and existing receipt-backed copy implementation.

The model supplies neither paths nor permission. Native code owns the measured
location bindings, checks directory identities again after inference, refuses
unknown fields/capabilities, validates dependencies and the completion predicate,
and checks explicit category, largest-selection and direction constraints.
Unsupported effects/constraints, negation, ambiguous plural references and
unsupported language must not silently become substitute copy goals.

Native routine permission remains default-off. The current grant revision,
optional source/destination scopes, privacy, protected paths, no overwrite,
20-file/100-MiB limit, 5% capacity budget and 1-GiB reserve remain mandatory.
An enabled collection grant avoids per-file approval; task text cannot enable
or broaden that grant. Permitted local paths are not restricted to known folders.

## Model/data boundary and bounds

Only the masked user goal, location IDs and minimal native binding descriptions
are sent to this model route. File content, directory names and private-location
observations are not supplied. The prompt separates the goal, native
observations and an empty untrusted-content field. An adversarial directory name
is data and cannot redirect the planner. Native tests cover this masking,
replacement of an observed directory and malformed plans.

Intent interpretation runs in its own chat history, restored afterwards. It has
no tool runner. The authenticated native transport rejects attempted tools and
oversized output. It retains request-epoch/privacy checks and task-specific Stop.
The agent has a 60-second total planning deadline; the native transport also has
a 90-second total watchdog. Interpretation permits two attempts, retaining the
same goal and bounded native consistency feedback. Writes are not retried or
replanned automatically. Missing authority/capability or verification failure
produces a limitation or recovery result, not a success claim.

CPU inference now explicitly uses four threads, with the existing context and
job-memory bounds. These measurements describe the reference machine, not a
universal performance guarantee.

## Frozen evaluation and real execution

The 36 authored cases and thresholds were frozen before implementation. The
case-file SHA-256 is
`00cf566d68b45394c303ebcfc9b1937b130008233e43c64e855ddba9825deac3`.
Targets: at least 22/24 supported requests correct, all 12 negative cases
non-executable, no location reversal, warm p95 no more than 30 seconds. The
additional zero-reversal gate was recorded before the first inference run.

Final run `intent-evaluation-05.json`:

- **24/24** supported requests correct; **12/12** negative cases blocked;
  **zero** location reversals.
- All 24 positive cases used actual pinned local Qwen inference. Negative cases
  were refused by the deterministic guard before inference; they are not a
  model-only safety score. All final accepted positive interpretations used one
  model attempt.
- Warm p95 **11,272 ms**, including every supported case. Model load/warmup is
  separately recorded. This is a normal development-process model evaluation,
  without tools, not the desktop sandbox or a filesystem execution benchmark.
- A separate opt-in native check independently compiled the actual model
  results. Each of the 24 supported plans executed real fixture copies with
  matching bytes, receipt verification, preservation of originals and verified
  undo. Negative plans remained non-executable. This adds execution evidence;
  valid JSON alone is not acceptance.
- Workspace regression: **383 native tests passed**, with **5 opt-in/environment
  tests ignored** by default; **138 JavaScript tests passed**. The model-to-file
  check and copied-runtime sandbox check also passed separately.
- Strict native static checks, frontend lint and build passed. The isolated
  runtime answered real questions and used authenticated tools inside the
  AppContainer without checkout dependencies.

These are small author-written regression cases, not independent unseen test
data or a statistical accuracy/security guarantee. Eight development cases were
available for prompt changes. The reserved wording was subsequently measured
and is now regression evidence, not a permanently unseen set.

Early failed/aborted runs remain in the evidence directory. The first one-shot
graph format was too slow and failed a basic supported goal. A prototype then
failed the selection-object contract. The next complete version met the gate
at 22/24, refusing two largest-image phrasings without copying. Development
vocabulary clarification produced the final 24/24 run. No cases, expectations
or thresholds were removed or lowered. A lint-only ASCII-check refactor retained
the same character predicate and passed regression checks.

## Laya decision

Laya remains **inactive**. No new Laya weights, training, runtime host or authority
were enabled. The existing calibrated-quality, independent-sample, benign-friction,
latency and packaging gates were not waived.

`laya-paired-replay.json` is a new analysis of preserved, paired actual-inference
rows from the earlier frozen 20-case advisory challenge. It verifies the input
case provenance and combines Qwen-only with Qwen-plus-Laya using restrictions
only. It runs no new inference and is not a joint-runtime benchmark. Its summed
latencies are estimates from separate recorded calls. This older label-only
Qwen judge is distinct from the new semantic file planner.

Both advisory combinations held all six eligible benign examples and allowed
none; adding Laya supplied no incremental execution benefit. Raw semantic
dangerous misses and per-case latency remain visible separately, so deterministic
denials do not conceal model mistakes. Twenty authored cases also cannot meet
the existing 300-independent-case requirements. Earlier calibration/latency and
quantization regressions remain unresolved. Neither model judge is therefore
introduced into live permission decisions.

## Remaining boundaries

The new route currently needs explicit quoted existing location bindings. It
does not invent paths, resolve arbitrary folder nicknames, create destination
trees, or guess what ambiguous “those” refers to. Existing singular file
references still use measured native search results and their existing checks.
Non-ASCII goal text is conservatively clarified by this route; multilingual
execution is not established. Unicode folder names are masked as data.

Most legacy tool routes retain their original parser/lifecycle. This increment
does not compose arbitrary settings, apps, reminders or content tools into one
general natural-language program. Real app control is Stage 4; broader file,
content and multi-app repair remain later stages. The app adapter remains mock.
Full security, signing, compiler hardening and clean installation remain open.
The historical Stage 0 installer does not contain this increment.

## Desktop acceptance history

The development walkthrough retained all 29 completed checks and captured
descendant cleanup. Its original process-completion response was not retained
across context compaction, so a numerical development exit code is not claimed.

The first optimized run (`desktop-production-01.txt`) failed with exit 1 during
the first restart: a captured WebView descendant did not exit within the
30-second cleanup wait. Eight checks had completed. A subsequent exact-PID
inspection found that process gone; no unrelated process was terminated.
The second unchanged run also failed during captured WebView cleanup. Both runs
are retained as failures, not counted as accepted desktop passes.

The fixture driver was then changed to release its retained accessibility
proxies while the provider is alive and terminate captured descendants in
descending tree depth before terminating their host. Pinned handles, measured
creation-time checks and mandatory kernel exit checks remain in place. No
application code, executable, runtime, acceptance expectation or timeout was
changed. This ordering addresses a suspected teardown interaction; the exact
Windows cause has not been established. A successful termination request alone
does not prove exit ([Microsoft API documentation](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess)).
The third run passed **29/29 checks** and exited **0**, including cleanup, on the
unchanged production hash in a fresh fixture. Its final exit code is persisted
separately. Reversed location order used actual local Qwen inference; native
execution copied the intended documents and preserved originals. The same run
verified restart, no overwrite, group undo, edited-output preservation,
task evidence, scoped grants, revocation, private zones and whole-system Stop.
The ordering change passed this run; broader shutdown reliability is not proven.

## Accepted artifacts and reproducibility

The tested executable is
`C:/Users/paagu/.codex/visualizations/2026/10/16/01a142c7-c123-7122-873a-8f2707fc4063/stage3-bundle/Custodian-stage3-production.exe`.
Its SHA-256 is
`1841cb92fc7d0c7e7948d3572b858e1595097003a84be5f91903c523b79889ad`.
It was built with `cargo build -p custodian-desktop --release --features tauri/custom-protocol`
and tested with the copied Stage 3 runtime beside it. Earlier accepted runtime
and source backups are preserved separately.

Evidence:

- [Frozen model results](EVIDENCE/stage-3/intent-evaluation-05.json),
  [independent result audit](EVIDENCE/stage-3/independent-result-audit.json),
  [actual native file execution](EVIDENCE/stage-3/model-native-execution.txt).
- [Accepted production walkthrough](EVIDENCE/stage-3/desktop-production-03.txt)
  and [persisted exit code](EVIDENCE/stage-3/desktop-production-03-exit.txt).
- [Artifact identities](EVIDENCE/stage-3/artifacts.json),
  [static artifact checks](EVIDENCE/stage-3/static-artifact-verification.json),
  [final result](EVIDENCE/stage-3/result.json).
- [Selected-source restore verification](EVIDENCE/stage-3/source-snapshot-verification.json).
  The capture verifies source/config/lockfile bytes and restores them into a new
  directory; it excludes dependencies, model files, generated output and general
  documents. It is not a self-contained or certified bit-reproducible build.

The historical installer remains Stage 0 and was not executed. Stage 4 has not
started. This stage result does not accept the entire offline product.
