# Laya integration plan — 17 October 2026

Status: planning only. No runtime integration, training, dependency installation,
permission expansion or new security certification is performed by this plan.
It refines the offline master plan and the original D-19 escalate-only rule.
Numbers below are proposed acceptance targets, not measured results.

Implementation update: the [first typed-model validation report](../verification/LAYA-VALIDATION-IMPLEMENTATION-2026-10-17.md)
records the implemented prototype and real CPU findings. The first candidate
does not meet activation targets; production integration remains disabled.
The [subsequent improvement report](../verification/LAYA-IMPROVEMENT-VERIFICATION-2026-10-17.md)
records structured effects, paired model comparisons and a faster experimental
runtime, including its new semantic regression. It does not waive any gate.

## 1. Outcome

Use the actual Laya typed decision model to detect questionable actions and
task drift while Custodian completes routine tasks automatically under standing
permissions. Demonstrate incremental benefit over deterministic rules and the
Qwen planner alone. Do not add a second model solely for the presentation.

Qwen interprets requests, plans and explains. Laya answers small, explicitly
defined questions about requests, proposed steps and permitted observations.
Native policy decides authority; the trusted executor performs and verifies
changes. Neither model grants permissions or changes standing preferences.

Laya can add restrictions. It cannot remove a native restriction. Its low-risk
answer is not evidence that an action is safe. Its absence must never silently
authorize work that required its screening. Non-generative outputs still permit
misclassification and adversarial manipulation.

## 2. Repository findings that must be resolved

| Area | Observed implementation | Required disposition |
|---|---|---|
| Original design | Master document Part XI specifies typed choice/score/yes-no models, local inference and six decision sites | Retain the division of responsibilities; narrow the first release to offline decisions |
| `services/mlhost/mlhost/laya.py` | Generates JSON through llama-cpp-python from filename, AV hits and signed flag | Treat as legacy metadata adviser, not as proof of actual typed-model integration |
| `model_loader.py` | Loads `laya.Q4_K_M.gguf`; no model manifest verification at this loader | Replace the active backend with a pinned, verified typed-model adapter |
| `training/generate_dataset.py` | Generates 90 templated examples, including approve/delete labels | Retire this dataset for the new decisions; preserve provenance, do not relabel it as validated safety data |
| Runtime policy | Adviser rejects approve/delete despite those training labels | Resolve training/inference schema mismatch before training |
| Tests | Fake generative responses exercise wrapper restrictions | Retain as legacy tests; add actual-model evaluation separately |
| Packaged desktop | Uses Qwen agent; Laya ML host is not part of its live reasoning path | Wire a supervised sandboxed host only after evaluation |
| Model manifest | Pins Qwen models, not the proposed typed Laya artifact | Add model, tokenizer, schema, calibration and runtime provenance |

The existing AV-hit quarantine output is an advisory label, not proof of
antivirus detection or an authorized quarantine operation. Supplied AV metadata
needs trusted provenance. This release will not turn it into automatic deletion
or quarantine.

## 3. Decision catalogue and order

| ID | Decision | Exact output semantics | First use |
|---|---|---|---|
| D1 | Semantic action risk | Multi-label flags: possible destructive effect, credential interaction, external disclosure, security-setting effect; explicit unknown | Every proposed mutation, using tool specification plus native facts; deterministic tool risk remains authoritative |
| D6 | Task alignment | within_request / ambiguous / outside_request, plus probabilities and abstention | Compare each mutation and cumulative plan with the trusted user goal and standing grant |
| D2 | Untrusted instruction attempt | Probability that text attempts to redirect the assistant; separate unknown state | Before untrusted names, descriptions or later permitted document excerpts enter model context |
| D5 | Intent routing | local_task / question / undo / settings / conversation / online_request / uncertain | Optional speed improvement after D1/D6 evaluation; never a safety boundary |
| D3, D4 | Outbound personal data and web side effects | Deferred | Online mode remains deferred; do not build unused online paths now |

D1 uses multiple flags because an action can be both destructive and externally
disclosing; these are not one ordered risk scale. D6 replaces a subjective
surprise score with a label rubric reviewers can apply consistently. Both
changes must be documented as refinements to the original design catalogue.

D2 is a warning signal, not a sanitizer or permission gate. A low score never
makes external text trusted. Quoted security examples are hard negatives.
No new content reader is enabled merely to demonstrate D2; synthetic replay
comes first, then approved readers when their sandbox/policy work is ready.

## 4. Automatic execution without per-file approval

The trusted UI establishes standing grants by operation type, permitted target
roles, exclusions, impact limits and recovery requirements. Whole-local-drive
reach is possible; fixed Documents/Downloads restrictions are not the design.
The model cannot establish or edit these grants. User task text supplies intent;
it does not silently amend persistent preferences.

Initial automatic mutation eligibility is deliberately precise: ordinary user
files, supported move/copy/rename/create-folder operations, no overwrite, no
protected system/application locations, verified destination capacity and a
prepared recovery record. Native policy must classify unsupported or ambiguous
locations as ineligible. Cross-volume move remains subject to integrity,
recovery and crash-safety gates. Settings need separate contracts and remain
under their existing approval flow until migrated. Undo must check current
privacy rules and actual recovery conflicts.

Suggested initial task limits are 100 changed files, 1 GiB transferred and no
more than 5 percent of the destination's measured free space, whichever limit
is reached first. These are adjustable conservative starting values, not proof
of safety; qualify them using workload evidence. Counters are cumulative across
replans/retries/substeps, with deduplication for idempotent retries. Splitting a
large operation into small steps cannot evade the task budget.

| Native decision | Laya result | Behaviour |
|---|---|---|
| Denied/private/protected | Any, including low risk | Deny before model observation or execution |
| Explicit approval required | Any | Retain approval requirement |
| Standing grant permits operation | Validated D1/D6 find no exception | Execute, verify and record without a permission prompt |
| Standing grant permits operation | Credible risk or task drift | Hold affected step and dependent steps; explain evidence and request clarification or additional permission when resolvable |
| Standing grant permits operation | Required screen unavailable, unknown or abstained | Hold mutations; retain planning and permitted reads; report screening unavailable rather than action dangerous |
| Permitted low-risk read | Laya unavailable | Deterministic-only read can continue; private exclusions still enforced |

An escalation is not an instruction to the executor. Laya may flag, but it may
not launch a tool, quarantine a file or solicit its own approval. High-confidence
injection detection quarantines the observation from planning, not the user's
underlying file. Clarification should be task-level and specific, not repeated
once per file. Never bundle unrelated exceptions into an opaque approval.

## 5. Integration contract and execution sequence

1. Capture the actual user goal and immutable tool schema; resolve references
   through native code. Stop/cancel and direct policy denial bypass all models.
2. Check privacy before reading features or forming a model request. Native
   facts include object class, operation effects, source/destination roles,
   identity versions, grant/policy versions, recovery readiness and task totals.
3. Construct a bounded state with trusted facts separated from untrusted text.
   Use opaque object IDs instead of full private paths. Pass only necessary
   permitted text. Do not accept the planner's claim that a file is ordinary,
   a change reversible, or a tool harmless as a verified fact.
4. Run D1 and D6 together where supported. D2 runs at observation ingestion.
   A full plan screen also checks cumulative effects and dependencies.
5. Native policy combines the result with the existing restrictions. A typed
   result can add caution but cannot turn deny/ask into automatic execution.
6. Immediately before each side effect, validate the task/grant/policy versions,
   cancellation, object identities, budget and unchanged assessment digest.
   Material changes require reassessment. Use the trusted executor; no second
   direct-write route. Fix the outstanding path-replacement boundary first.
7. Execute, verify actual effects and persist recovery evidence. Qwen explains
   measured results, not merely Laya's prediction or its own success claim.

Request envelope: protocol/schema versions, request/task IDs, policy/grant
revisions, goal digest, plan/step digests, native facts, provenance-tagged text,
required decision IDs, deadline and model/schema/calibration IDs.

Response envelope: matching IDs/digests, per-decision labels/distributions,
abstention, input token count, truncation status, model/calibration IDs and timing.
Validate finite numbers, ranges, probability normalization, required labels,
lengths and freshness. Reject stale/mismatched/replayed responses and malformed
output. Authenticate IPC using fresh per-launch credentials, bounded frames and
the existing framing primitives. Authentication proves origin, not correctness.

Do not silently truncate away the user goal or native facts. Oversize input
abstains or uses validated chunking with coverage recorded. Cache only exact
state/schema/model/calibration/revision digests; no approximate safety cache.
Results expire at the task boundary or on any relevant state change.

## 6. Actual model and runtime selection

Evaluate the official typed-decisions checkpoint for English and multilingual
checkpoint for explicitly supported additional languages. Start with one
checkpoint resident; add a second only if measured accuracy justifies memory
and loading costs. Do not translate unsupported languages and assume equivalent
safety performance. Abstain explicitly outside evaluated language coverage.

Pin upstream code and checkpoint revisions, licenses and every model/tokenizer
asset. Verify the pinned SDK's exact local-loading, schema, probability,
abstention and export APIs in an isolated prototype. Use the Python reference
implementation as the correctness baseline; test INT8 ONNX on Windows CPU as
the deployment candidate. Do not assume export parity or published GPU latency.

The supervised ML host has no network capability, no arbitrary user-file roots,
no writer authority, bounded memory/CPU/concurrency and a killable job. Only
verified runtime/model assets are readable. Build-time downloads are allowed
only in the development workflow; the shipped runtime must never fetch a
missing checkpoint. Missing/corrupt assets disable the relevant screen.
No HTTP server, MCP server or unrestricted-process fallback is shipped.

Target: warm p95 combined D1/D6 latency <= 500 ms for a representative step,
p95 D2 <= 1 second for the chosen bounded input, and additional resident memory
<= 1 GiB on the actual demonstration laptop. Record cold start separately;
initial target <= 15 seconds. Use a bounded queue and 2-second warm timeout.
These are provisional usability gates: measure alongside Qwen and disk work,
including peak memory, before committing them. Stop must not wait for inference.

## 7. Data, training and calibration

For each D1, D6 and D2 decision, collect initially 3,000 synthetic/public cases:
1,800 train, 400 development, 400 calibration/threshold selection, 400 frozen
test. Group by scenario/template family and document source BEFORE splitting;
translations, paraphrases, filename variants and related traces stay together.
Document public-data licensing; use no private user content for cloud training.
These are starting budgets, not sufficient evidence for all languages/subgroups.

D1 includes compound effects, misleading tool descriptions and incorrect
reversibility claims. D6 includes valid implied substeps, pronouns, changed
goals, multi-step detours and cumulative bulk effects. D2 includes quotation,
code samples, filenames, mixed scripts, indirect instructions, encodings and
attacks split across adjacent chunks. Missing identity/evidence is unknown,
not benign. Labels describe effects and task alignment, never approve/delete.

Write a decision rubric before generation. Teacher labels are proposals.
Independently review every frozen test label and every adversarial case; a
second reviewer resolves ambiguous/high-impact cases and audits at least 20
percent of remaining training labels. If a second reviewer is unavailable,
declare that limitation; do not claim independent validation. Ambiguous cases
receive an explicit unknown/clarification label rather than invented certainty.

Compare deterministic rules, Qwen-as-judge, base Laya and domain-adapted Laya.
Train only after baseline errors establish where specialization is needed.
Use the official pinned training schema, not the obsolete chat-message dataset.
Calibrate after fitting; select per-decision thresholds on the dedicated
calibration split. Never tune thresholds on frozen test results. If the test
reveals a failure, preserve it, revise on development data and version a new
unseen test set for the next acceptance claim.

Do not use self-reported generative confidence. Measure calibrated distributions
with reliability plots, Brier score, ECE, class counts and confidence intervals.
Calibration does not establish out-of-distribution correctness.

## 8. Acceptance and evidence

Proposed activation gates, evaluated per decision and supported language:

| Gate | Required evidence |
|---|---|
| Boundary correctness | Zero forbidden native grants across hostile labels, crashes, timeouts, replay, changed policy, private aliases and model absence; fixtures and fuzz/property tests; no statistical security guarantee claimed |
| Dangerous-case sensitivity | At least 300 independent critical test cases for D1, 300 off-task cases for D6 and 300 injection cases for D2; recall >= 98%, one-sided 95% lower confidence bound >= 95% |
| Routine usefulness | Unnecessary holds/clarifications <= 5% on eligible benign tasks; abstention and false alarm rates reported separately and together |
| Calibration | Per-decision ECE <= 0.05 with disclosed bins/counts and reliability plots; Brier score no worse than uncalibrated candidate |
| Quantization | Deployment artifact passes the same safety/usability gates and loses <= 1 percentage point critical recall vs reference; paired disagreements reviewed |
| Performance | Meet measured CPU latency/memory budgets above with Qwen active; no cancellation blockage |
| Incremental value | Combined system beats rules-only and rules+Qwen on a paired frozen challenge suite without violating benign friction limits; if benefits overlap or disappear, do not claim a gain |
| Offline packaging | New profile with networking unavailable, empty external model caches and missing/corrupt asset cases; no attempted runtime fetch or unsandboxed fallback |

Expand the frozen test sets to meet critical-case counts; a nominal 400 total
cannot supply every required stratum. Report both balanced challenge results
and a representative task mix; do not infer real-world precision from an
artificially attack-heavy set. Zero misses on a finite test is not zero risk.
Add dependent-operation and cumulative-impact integration cases independently
of per-step classification scores.

Shadow mode first: Laya records advisory outcomes on synthetic replay, then on
opt-in permitted live tasks without changing execution. Shadow mode retains
existing per-operation approvals. Only after the execution contracts AND model
gates pass does a limited standing-grant mode execute automatically. Roll back
that mode on regression; never silently drop its required screens to keep it
running. Each decision activates separately; one failure does not disable chat.

Persist compact decision receipts: task/step digests, native checks, model and
calibration hashes, flags, confidence, abstention and actual outcome. No raw
document content or full paths by default. Apply privacy revocation and bounded
retention to these records too. User-facing reasons come from structured facts
and fixed reason codes, not invented model explanations. No green "safe" badge.

## 9. Work packages, dependencies and deliverables

| Order | Work package | Deliverable / completion condition |
|---|---|---|
| L0 | Reconcile identity and architecture | ADR distinguishing official typed Laya from legacy GGUF shim; decision schemas; dataset deprecation note; baseline inventory |
| L1 | Runtime feasibility | Isolated real-model adapter, pinned assets, actual Windows CPU benchmark and reference/export comparison; no desktop authority changes |
| L2 | Decision specifications and datasets | Versioned D1/D6/D2 rubrics, provenance, grouped splits and independent label review; schema validation |
| L3 | Baselines, adaptation and calibration | Reproducible evaluation scripts, frozen reports, thresholds and model card; retain failed candidates |
| E1, parallel prerequisite | Native task contracts | Standing-grant UI/storage, one mutation authority, cumulative budgets, handle-based race hardening, recovery/cancellation and revocation tests |
| L4, after L1 | Sandboxed host and IPC | Verified assets, bounded authenticated protocol, restart/error handling and offline failure tests |
| L5, after L3/L4 | Shadow integration | Screens at native step preparation/observation ingestion; receipts and actual-vs-predicted comparison; existing approvals preserved |
| L6, after L5/E1 | Bounded automatic execution | Eligible tasks run without per-file prompts; exceptional steps stop with specific reasons; final checks rerun immediately before action |
| L7 | Product evidence | Packaged demonstrations, paired ablation report, measured CPU budgets and honest capability documentation |

Expected code touchpoints: new typed adapter and decision contracts under
`services/mlhost/mlhost/`; replace active `model_loader.py` semantics; versioned
training/evaluation tools under `training/`; native decision client and policy
composition beside `assistant/privacy.rs` and `assistant/file_ops.rs`; core
task/grant contracts near `file_tasks.rs`; trusted settings UI for standing
permissions; runtime staging and manifests. These are proposed locations, not
an instruction to spread authority into each caller. All mutations ultimately
use the same trusted execution boundary. Preserve unrelated dirty-checkout work.

First implementation unit: L0 + L1, followed by the D1/D6 dataset baseline.
Do not start with fine-tuning, automatic quarantine, web support or removing
approval checks. Decisions become active only when their dependencies pass.

## 10. Demonstration that strengthens the project

1. Establish standing permission once for ordinary reversible file organization
   across permitted local drives; retain protected/private exclusions.
2. Request a realistic synthetic project organization task; eligible steps run
   without individual approvals and produce measured results/recovery receipts.
3. Present a planted off-task step involving an ordinary permitted file. Show
   that it passes basic path permission checks but D6 flags task mismatch.
4. Replay a malicious permitted document/name example. Show D2's signal and the
   structural separation that prevents content from granting authority. Low
   detection scores never bypass those controls.
5. Change permissions mid-task; stale inference and remaining mutations stop.
6. Restart and recover the completed supported changes with matching bytes.

Publish rules-only, Qwen-assisted and Laya-assisted results: critical misses,
unnecessary holds, abstention, task success, user prompts per successful task,
latency and memory. The defensible contribution is measured local semantic
screening combined with enforceable autonomous execution. It is not "Laya
makes any action safe" or "we built/trained the upstream model."

## Sources checked for this plan

- [Official Laya repository](https://github.com/NandhaKishorM/laya): typed
  inference interfaces, local loading/export and domain adaptation. Pin an exact
  revision during L1; a live README is not an installed dependency lock.
- [Typed-decisions model card](https://huggingface.co/convaiinnovations/laya-typed-decisions)
  and [multilingual model card](https://huggingface.co/convaiinnovations/laya-multilingual):
  candidate checkpoints, not Custodian accuracy evidence.
- [OWASP agent security guidance](https://cheatsheetseries.owasp.org/cheatsheets/AI_Agent_Security_Cheat_Sheet.html):
  independent authorization and constrained tools.
- [Scikit-learn calibration documentation](https://scikit-learn.org/stable/modules/generated/sklearn.calibration.CalibratedClassifierCV.html):
  separate fitting and calibration data; no specific implementation selected here.

Related local documents: `OFFLINE-HACKATHON-MASTER-PLAN.md`, master design Part XI,
and `OFFLINE-FOUNDATION-IMPLEMENTATION-2026-10-16.md`. The old master claim that
an escalate-only model cannot make the system less safe is too broad: it can
still cause denial of service, reveal inputs through mishandling, or fail to
catch risk. Escalate-only prevents permission expansion; the remaining risks
require their own tests and controls.
