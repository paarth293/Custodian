# Retained verification evidence

This directory contains deliberately retained acceptance inputs, expectation
locks, artifact inventories, scoped results, and supporting run records.
Several verifiers and model evaluators read files here directly. It is source
material for reproducible verification, not a disposable cache.

Keep frozen cases and their locks together. Preserve failed attempts alongside
successful attempts when a report relies on them. Do not rewrite a historical
result to describe a different binary or current working tree, and do not
reformat locked JSON merely to change its appearance.

New ad-hoc logs, screenshots, runtime state and large exports belong in ignored
`artifacts/evidence/` or `target/` directories. Before promoting selected
evidence, review it for credentials, user content and machine-specific details,
record the source/artifact identity, and link it from an implementation or
verification report. Do not add models, installers, recordings, caches or
generated fixture trees here.

The October 19 cleanup retained every JSON input, lock, result and inventory
unchanged, as well as raw evidence named in maintained reports or source.
It archived 303 unreferenced raw logs under the ignored local directory
`.local/repository-archive-2026-10-19/docs/EVIDENCE/`; its manifest records the
original paths and SHA-256 hashes. Historical inventories may still describe
those original run paths. They record past runs and must not be rewritten to
pretend that they describe the current tree. The local archive is not available
in a fresh clone, and is not required by the retained executable evaluators.

Do not commit complete run directories. Retain the frozen inputs and a scoped
result, then select supporting logs only when a maintained report cites them.
