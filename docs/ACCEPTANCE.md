# Completion means verified behavior

The target is the user journey in the offline-assistant roadmap: delegate a task
in ordinary language, continue working, see measured and verified results, and
have a dependable way back. A successful build or simulated workflow does not
establish product completion.

Backend tests, model evaluations, packaging checks and interactive desktop
checks are different evidence and cannot substitute for one another.

| Requirement | Required evidence | Current evidence, 16 October 2026 |
| --- | --- | --- |
| Offline chat responds | Bundled pinned model answers inside the default AppContainer within the existing 120-second inactivity deadline | Real CPU-model smoke passes, including a tool request; broader reliability and latency remain open |
| Requests select the intended tools | Unchanged full evaluation scores at least 95%, without rounding up; all designated deletion-claim checks pass | CPU 1.5B and Vulkan 4B each score 56/57 (98.25%), with 4/4 designated deletion-claim checks; desktop results are simulated |
| Find the requested file | Actual path, measured bytes, scope and exclusions, in fixtures and the live desktop | Backend tests and live custom-folder measured search pass; whole-PC coverage and exclusions are described in the current report |
| “Move it” refers to the displayed file | Resolve a measured result; refuse ambiguity, replacement and stale results after conversation reset | Backend refusal tests and live “move it” journey pass |
| Approval binds the shown change | Absolute source/destination, single-use expiring card, source identity/content and destination parent checked again | Fixture tests pass for changed source, identical-byte replacement, replaced parent and occupied destination. General path-race hardening remains open |
| File tasks execute and verify | Actual fixture mutations, hashes, no overwrites, durable prepared record before mutation | Move/copy/rename/folder creation and refusal tests pass; actual cross-drive desktop approval and verified move pass |
| Restart preserves task truth and undo | Reopen records, report interrupted tasks honestly, verified undo and changed-target refusal | Fixture tests and live restart/undo pass; abrupt process death during mutation remains open |
| Independent work and Stop work | Overlapping live tasks, independent status, cancellation, no late action after Stop, fresh conversation ignores stale events | Backend and frontend tests pass; live C:/D: scan, independent storage answer and Stop pass. Repeated walkthrough acceptance remains open |
| Installation works offline | Node, agent, dependencies and pinned model included; isolated runtime check; clean-profile install, actual tasks and uninstall | Runtime staged with these assets; isolated and installer outcomes belong in the current verification report |
| Broader agreed capabilities work | Each supported capability executes and verifies; unsupported capabilities explain their limitation | Online browsing, installation workflows, parser/semantic search, voice and other documented placeholders remain open |

The product is **not accepted as complete** while a required row remains open.
Repeated UI acceptance must include the original offline walkthrough five
times, as required by build-plan Step 103, and the newer file-task journey.
Destructive tests use fixtures; user files are not acceptance fixtures.

## Evidence rules

- Record commands, outcomes, model and environment. Preserve failing cases and
  their original expected answers.
- Mark checks not run when they were not run. Generating an installer does not
  prove installation; a transport ping does not prove inference readiness.
- Model metrics describe their harness. Deletion-claim checks are not a general
  security evaluation.
- Verify the shipped CPU model and optional GPU model separately. Passing with
  the development GPU model does not accept the bundled CPU experience.
- Retain interrupted-operation evidence. Never replay changes automatically
  after restart or claim completion without verification.

See [the roadmap](CUSTODIAN_OFFLINE_ASSISTANT_IMPLEMENTATION_ROADMAP.md),
[current verification](VERIFICATION-2026-10-16.md),
[the open findings](FIX-INVENTORY-2026-10-15.md), and
[implementation limits](../HONESTY.md).
