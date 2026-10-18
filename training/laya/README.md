# Typed Laya validation

This prototype evaluates official typed Laya separately from desktop authority.
It does not enable automatic execution or certify security. The existing
`mlhost.laya` GGUF metadata adviser remains legacy and disconnected.

`training/generate_dataset.py` and its 90-example dataset are obsolete for this
workflow: their approve/delete labels conflict with today's advisory contract.
Do not use them to train these decisions. New examples use goal, step and
native facts, with multi-label risk and task alignment annotations.

The checked-in development cases are manually authored synthetic smoke cases,
not independent held-out validation. The evaluator reports every miss and
abstention; a successful run means inference completed, not deployment approved.
No calibrated threshold or standing permission is enabled by this prototype.

The [first real CPU findings](../../docs/LAYA-VALIDATION-IMPLEMENTATION-2026-10-17.md)
include substantial classification misses and latency/memory above the proposed
targets. Keep this candidate disconnected from production authority.

Model assets are fetched during development using the revision/hash lock.
Inference uses a verified absolute local directory with Hugging Face offline
flags. The Python prototype itself is NOT an AppContainer or network sandbox;
OS isolation and authenticated supervised IPC are later milestones.

## Reproduce the development run (PowerShell, repository root)

```powershell
python scripts/fetch_laya_validation.py --sdk-archive target/laya-validation/laya-source.zip
python -m venv --system-site-packages target/laya-validation/venv
$env:SETUPTOOLS_USE_DISTUTILS = 'stdlib'
target/laya-validation/venv/Scripts/python.exe -m pip install --no-deps --no-build-isolation target/laya-validation/laya-source.zip
$env:PYTHONPATH = 'services/mlhost'
target/laya-validation/venv/Scripts/python.exe -m mlhost.laya_eval --model-dir target/laya-validation/model --output target/laya-validation/development-report.json
```

This environment reuses the development machine's existing torch, transformers,
safetensors, NumPy and Hugging Face libraries. It is not a standalone deployment
lock or a clean-machine install recipe. Package versions are recorded in the
report. The stdlib setting works around this machine's setuptools conflict;
prefer a fully locked isolated environment when preparing a shipped runtime.

Download is explicit and confined to the development fetch tool. Inference
forces offline Hub flags and a Python socket audit guard. That guard is useful
diagnostic evidence, not OS network isolation. The report records attempted
Python socket operations, loading warnings, cold forward, warm-case timings and
peak process working set (including libraries; not incremental model memory).
No production timeout/kill supervision is claimed for this prototype.

Run wrapper checks with `python -m pytest services/mlhost/tests -q`.
Actual-model inference is deliberately not part of routine CI or unit tests.
Evaluation exit 0 means all cases returned structurally valid predictions;
wrong predictions remain visible in the report and never approve deployment.

## Structured comparison experiment

`comparison.jsonl` is a separate, author-reviewed 20-case synthetic development
suite. Its explicit reference labels are not computed from the rule catalogue.
It is not independently reviewed or a deployment holdout. The generator refuses
to overwrite an existing suite. Ground-truth fields never enter model inputs.

`mlhost.action_screen` supplies a prototype tool-effect catalogue and checks
synthetic object identities. These are Python evaluation rules, not newly wired
Windows-native enforcement. Real native feature extraction remains a prerequisite.
Unknown tools/facts stay unknown; a semantic label cannot clear a native conflict,
known risk or unknown state. None of these results grants permission.

```powershell
$env:PYTHONPATH = 'services/mlhost'
target/laya-validation/venv/Scripts/python.exe -m mlhost.laya_compare --model-dir target/laya-validation/model --runtime-probe --dynamic-int8 --output target/laya-validation/comparison-v2.json
target/laya-validation/venv/Scripts/python.exe -m mlhost.laya_compare --model-dir target/laya-validation/model --reference-report target/laya-validation/comparison-v2.json --dynamic-int8 --encoder-only --output target/laya-validation/comparison-encoder-int8.json
```

Reference reuse requires identical cases, model ID, state digests and question
schemas. Reference results are checkpointed before experimental quantization;
an interrupted candidate is marked pending. Runtime tuning uses an old development
case, not the new comparison labels. Thread probes have only two measured samples
per condition, so treat their timings as exploratory. INT8 candidates are
ephemeral in-memory copies, never overwritten model assets or approved artifacts.
The PyTorch quantization API used here is deprecated; a shipped implementation
needs a supported runtime and parity validation.

The Qwen comparator is `services/agent/eval/laya-comparison.mjs`. It checks the
existing GGUF pin, uses CPU/no tools and constrains output to three labels. Pass
the case file, a JSON export of `ALIGNMENT_QUESTIONS`, and an output file. Optional
`--format narrative` tests explicit text framing instead of a JSON-only prompt.
Both are development prompts, not the desktop planner's actual workflow.
`mlhost.laya_qwen_report` combines those labels with the same rules and refuses
partial/mismatched case sets. Raw model accuracy remains separately visible.
