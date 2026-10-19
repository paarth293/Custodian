# Typed Laya validation implementation — 17 October 2026

The first development validation slice is implemented. The actual upstream
typed Laya model runs locally through a new adapter and a reproducible synthetic
evaluation command. It is not connected to desktop permissions or execution.
The measured candidate does not meet the proposed accuracy/performance targets.

Follow-up: [structured rules, selective inference, paired comparisons and
encoder INT8 experiments](LAYA-IMPROVEMENT-VERIFICATION-2026-10-17.md) improve
measured runtime and retain all failed candidates. Activation remains blocked
by quality/performance gates; the original findings below are preserved.

## Implemented scope

- `services/mlhost/mlhost/typed_decisions.py`: independent typed-model adapter,
  four action-risk flags and task alignment. Fixed question schema, bounded
  goal/step/native-facts state, state/schema/model digests, advisory-only output.
- `training/laya/assets.lock.json`: six exact model asset hashes and sizes,
  upstream model revision, SDK revision/version and source archive hash.
- `scripts/fetch_laya_validation.py`: explicit development-only fetch with hash
  verification, bounded downloads and refusal to overwrite differing assets.
- SDK loading verifies local model assets and installed SDK archive provenance,
  forces local CPU/eager inference and Hub offline flags, and uses no Router or
  model-download fallback. Missing/modified assets fail before SDK import.
- Prediction validation rejects incomplete decisions, invalid probabilities,
  unknown labels, inconsistent distributions, truncation and missing coverage.
  Inference failures abstain. No result grants a permission or executes a tool.
- `mlhost.laya_eval`: development CLI with per-case predictions, honest
  abstention denominators, diagnostic errors, cold/warm timing and peak memory.
  The final CLI also records complete runtime initialization, loading warnings
  and attempted Python socket operations under a Python-level audit guard.
- Twelve manually authored synthetic development cases: ordinary requested
  operations, necessary substeps, ambiguous reference, unrelated ordinary-file
  changes, credential access, disclosure, security settings and compound risk.
- Architecture decision record distinguishes actual typed Laya from the old
  GGUF adviser. The old training generator is explicitly deprecated for new
  decision training; its historical dataset is preserved.

This implements L0 and the reference-runtime/evaluation portion of L1 in the
[integration plan](../plans/LAYA-INTEGRATION-PLAN-2026-10-17.md). ONNX/export comparison,
domain adaptation, held-out validation and all production activation gates
remain open. D2 suspicious-content detection is not implemented in this slice.

## Real CPU findings

Evidence: [unaltered full development report](../EVIDENCE/laya/development-baseline-2026-10-17.json).
Hardware: AMD Ryzen 5 3550H, four physical cores/eight logical processors;
inference configured for four CPU threads. Eager reference runtime, Qwen not
running as part of this benchmark. This is not a combined desktop load test.

| Decision | Correct cases | Critical positives missed |
|---|---|---|
| Destructive effects | 12/12 | 0/3 |
| Credential interaction | 11/12 | 1/2 |
| External disclosure | 10/12 | 2/2 |
| Security-setting effect | 8/12 | 0/1 |
| Task alignment | 5/12 | See per-case predictions; no deployment threshold selected |

All 12 cases returned structurally valid predictions, with zero wrapper
abstentions. That establishes API compatibility, not safe decision quality.
Risk flags use a diagnostic 0.5 cutoff only. No thresholds were tuned after
seeing these cases, and the question schema was not changed to improve scores.
This tiny authored set is not independent validation or a general accuracy
estimate. No rules/Qwen ablation or incremental-benefit claim is made yet.

- Warm-case median for all five questions: **13.47 seconds**.
- Warm-case p95: **19.78 seconds**, using nearest-rank over 12 cases.
- Local asset verification plus SDK/model load: **172.10 seconds**. This first
  report excludes torch import from its load timer, so it understates complete
  startup; the final diagnostic CLI measures initialization separately.
- Peak process working set: **2,983,067,648 bytes**, approximately 2.78 GiB.
  This includes interpreter, libraries and temporary load allocations; it is
  not incremental resident model memory. Sustained memory was not benchmarked.

The checkpoint's SDK emitted a warning about an out-of-range `choice:11+`
temperature being clamped from approximately 0.10058 to 0.5. These questions use
binary and three-option decisions, but retain the warning rather than claim
all upstream calibration entries are valid. No Custodian-specific calibration
has been performed. Do not attribute the observed misses to this warning
without evidence.

The [final one-case diagnostic](../EVIDENCE/laya/offline-audit-2026-10-17.json)
re-ran the requested-move case through the finalized CLI with the Python socket
audit guard enabled. It recorded zero attempted guarded Python socket events,
one structurally valid prediction, 66.59 seconds initialization including torch,
53.83 seconds asset/SDK/model loading within that initialization, 14.65 seconds
for the measured post-warmup screening call, and 2,983,452,672 bytes peak working
set. This explicitly limited one-of-twelve run verifies the final harness and
records the loading warning; its correct result does not replace the twelve-case
findings or establish network isolation against native code.

## Verification and provenance

The Python ML-host and end-to-end suite passes: **43 tests**, including 22 new
typed-adapter tests. Ruff checks/formatting and targeted mypy checks pass.
Tests cover bad distributions, unknown labels, missing/truncated coverage,
changed/missing/extra assets, SDK provenance, offline loading arguments and
failure handling. They are wrapper tests, not real-model accuracy evidence.

Model: `convaiinnovations/laya-typed-decisions`, revision
`e929ae5cf69bc34259cd2f95c9e91145b818b1f0`.
SDK: Laya 0.4.8, source revision
`4ac226d5ee88ed6df8db288a05c766d7a34ee682`.
Model weights SHA-256:
`4fa56de72383a9d3efa9cfa78955733c81b9fc8067a587ca4beb82c78107a24e`.
Runtime versions are recorded in the evidence report. This workspace prototype
uses a virtual environment reusing existing ML libraries; the entire transitive
runtime is not yet locked or bundled. No global package replacement was needed.
A local setuptools conflict was resolved with the stdlib distutils setting
documented in the reproduction instructions.

Reproduction: [validation README](../../training/laya/README.md).
Transient logs and local model/runtime assets are under `target/laya-validation`.
Inference uses synthetic text only and does not inspect user task files.

## Activation decision

**Do not activate this candidate for autonomous execution.** It fails the
proposed task-alignment, disclosure detection and latency targets; peak process
memory also exceeds the provisional 1 GiB envelope, though sustained and
incremental memory require separate measurement. Do not weaken gates or hide
mistakes to make integration appear complete.

Next: create the specified reviewed domain dataset and compare rules, Qwen and
Laya; evaluate an optimized/quantized Windows CPU runtime against the reference
outputs. If quality and performance do not improve enough, leave that decision
disabled. Independent native task contracts, standing grants, race hardening,
recovery and authenticated sandbox supervision remain prerequisites regardless
of model choice. The current desktop retains its existing approval behaviour.

Neither this normal-process prototype nor Python's socket guard is an OS
security boundary. Same-user replacement races, private-data feature extraction,
production timeouts, cancellation supervision and live policy revocation have
not been certified here. No new installer or production security claim results
from this development-only milestone.
