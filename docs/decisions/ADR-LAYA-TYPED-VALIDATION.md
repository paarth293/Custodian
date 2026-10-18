# Typed Laya validation boundary

Date: 17 October 2026. Status: implemented for development evaluation only.

The original design's Laya is a non-generative typed decision model. The
existing `mlhost.laya` metadata adviser uses a generative GGUF model and must
not be represented as that integration. Preserve that legacy interface while
introducing `mlhost.typed_decisions` as an explicit separate prototype.

The prototype loads the official typed-decisions checkpoint from verified local
assets, never a Hub identifier at inference time. Pin the upstream source archive
and model assets separately from generated output. Require SDK install provenance,
offline environment flags, complete coverage evidence and bounded typed results.
No inference result contains an approval or execution command.

First evaluation covers four independent risk flags and task alignment. The
cutoff of 0.5 is diagnostic only. Confidence gating at zero explicitly requests
full predictions to measure errors; it is not a production acceptance threshold.
Upstream temperatures are retained but are not domain calibration evidence.

The development evaluator runs as a normal user process, not an AppContainer.
It accepts synthetic goal/step/facts text only and never opens task file paths.
It has no connection to the desktop's permission or mutation routes. Authentication,
supervision, deadlines, OS sandboxing, real native facts, standing permissions and
desktop integration remain later milestones. Offline library flags are not an
OS network isolation boundary. Same-user replacement races are not solved by
asset hashing. This is not a shipped security service.

Activation requires the gates in the Laya integration plan. A finite smoke run
or passing wrapper tests cannot satisfy those gates. Retire the old generated
approve/delete training examples for new decision training, preserving historical
provenance. Add no automatic approval or quarantine based on this prototype.
