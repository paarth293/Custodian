# Current checkout verification, 19 October 2026

Status: audit completed; **the whole product is not accepted**.
This report records fresh execution separately from historical stage evidence.
The working tree contains existing user changes; no reset or commit was made.

## Findings established so far

The claim that every stage is implemented is contradicted by the implementation
and the Stage 5, Stage 6 and full-roadmap delivery records. Core APIs, UI routes,
real application execution and installed-product acceptance are separate gates.

| Stage | Current disposition | Remaining acceptance or implementation |
| --- | --- | --- |
| 0: baseline | Existing frozen requirements and fixtures; current register repaired and validated | Current delivery/runtime and clean-install checks are separate |
| 1: authority/recovery | Historically accepted bounded foundation; current native regression passes | Full cross-route security and later capability coverage remain open |
| 2: coordinator | Existing bounded coordinator and receipt-backed file composition | General heterogeneous task composition is not established |
| 3: language/planning | Constrained file planning executes in the fresh desktop walkthrough | General language scope limited; Laya inactive; fresh free-form model correctness check failed |
| 4: real applications | Narrow VS Code text and Excel workbook adapters | Repeated optimized desktop and disposable installation gates open; installed Excel version now outside qualification |
| 5: files/storage | Archive, batch, backup/restore and governed recycle core modules | Archive/backup/recycle desktop integration, qualified Windows recovery provider and full failure drills missing |
| 6: grounded content | Bounded TXT/Markdown/CSV core; narrow existing workbook/editor subsets | Contained content worker, desktop planning, DOCX/PDF/PPTX, image/OCR/media and multi-app showcases missing |
| 7: settings/diagnostics | Catalogued per-user setting and revert foundation | Broader hardware/device/diagnosis adapters and live acceptance missing |
| 8: development/software | Legacy helpers do not establish a supported desktop capability | Contained project commands, editing/Git and software lifecycle acceptance missing |
| 9: routines | Task store and bounded largest-file backup exist | General durable scheduling, reusable workflows and local communication artifacts incomplete |
| 10: interaction/resources | Progress, Stop/Pause and recovery UI exist | Full accessibility, multilingual and reference-hardware acceptance incomplete; voice deferred |
| 11: adversarial acceptance | Individual authority/privacy/path/process tests exist | Full cross-route adversarial acceptance incomplete |
| 12: installed product | Runtime and package tooling exist | Disposable offline install/upgrade/uninstall, independent tasks and beginner acceptance missing |

Stage 0–3 historical acceptance applies only to the declared increments. This
table does not revoke valid historical results or treat them as current whole
roadmap acceptance. Online services remain intentionally deferred.

## Fresh checks

Raw working logs are under `target/audit-2026-10-19/`. A retained copy of the
audit logs, manifests and outcome is under
[`docs/EVIDENCE/audit-2026-10-19/`](../EVIDENCE/audit-2026-10-19/).

| Check | Result | Evidence |
| --- | --- | --- |
| Native workspace tests, locked/offline/serial, normal Windows profile | 475 passed, 0 failed, 8 ignored | `rust-tests-normal.txt` |
| JavaScript workspace tests | 144 passed (27 desktop, 104 agent, 7 broker, 6 editor) | `js-tests.txt` |
| Python configured suite | 70 passed | `python-tests.txt` |
| Additional Cedar fixture tests | 2 passed | `cedar-tests.txt` |
| TypeScript type check | Passed | `typecheck.txt` |
| ESLint | Passed | `lint.txt` |
| Workspace/frontend production JavaScript build | Passed | `js-build.txt` |
| Strict all-target Rust Clippy | Passed; incremental hard-link fallback warnings retained | `clippy.txt` |
| Rust formatting | Passed | `rust-format.txt` |
| Python mypy | Passed, 32 source files | `python-types.txt` |
| Secret scan | 0 findings in this scan | `secrets.txt` |
| Python lint/format after correction | Passed | `python-lint-final.txt`, `python-format-final.txt` |
| Prettier after correction | Passed | `js-format-final.txt` |
| Requirement/source/evidence/dependency/frozen-acceptance register | 58 requirements, 17 routes, 15 frozen cases passed | `register-render-02.txt` |
| Historical-model replay through current native compiler/file executor | Passed: 36 frozen cases, 24 real file compositions and undo; no fresh inference claimed | `historical-intent-native-replay.txt` |
| Installed unqualified Excel refusal | Confirmed before Office launch | `excel-version-refusal.txt` |
| Current optimized native production build | Passed, 15 minutes 23 seconds; Tauri production protocol enabled | `production-build.txt` |
| Runtime staging and outside-checkout pin checks | Passed: pinned model/worker, 67 agent modules, 3 editor payloads, 133 runtime packages | `runtime-stage.txt`, `runtime-pins.json` |
| One-shot worker fault matrix and optimized worker outside checkout | Passed: 16 fault modes and release worker qualification | `worker-qualification.txt` |
| Persistent worker fault/Stop/revocation/cleanup matrix | Passed: 10 fault modes | `persistent-worker-qualification.txt` |
| Isolated packaged model smoke | **Failed semantic correctness** after successful isolated load/inference | `isolated-runtime.txt` |
| Optimized desktop baseline outside checkout | Passed, all 29 checks, including actual Qwen-to-native file workflows | `desktop-baseline-01.txt` |
| Repeated editor/Excel desktop application checks | **Failed**: three editor successes, fourth failed cleanup; fifth and Excel phase not reached | `desktop-apps-01.txt`, `editor-failure-state.json` |

The initial restricted native run failed on Windows DPAPI access with OS error
5. The complete normal-profile rerun passed without weakening that test. Its
failed log (`rust-tests.txt`) is preserved. The eight default ignored tests are
not counted as passing; they require explicit model/runtime/worker fixtures or
read the machine's Start menu. CI checks do not establish absence of all defects.

## Corrections made during this audit

The capability register referred to `stage-5-execution` and
`stage-6-execution` without defining their evidence links. Both links now resolve.
The generated register was refreshed without modifying frozen acceptance tasks.
App/privacy/content route descriptions now identify current narrow adapters and
unconnected content code. D03 and S03 describe partial implementation, rather
than claiming that no adapter or application exclusion exists.

Prettier failures in README and editor-adapter tests and Python import/format/
line-length failures in fixture/delivery scripts were corrected. Pre-fix bytes
are retained in `target/audit-2026-10-19/pre-fix-files.json`. These changes do not
add missing product features or alter expected test results.
The register generator also emitted an extra blank line at EOF, causing a new
`git diff --check` failure after regeneration; its output now ends with one
newline. The regenerated register validates and the diff check passes.

## Repeated editor failure

The current optimized desktop completed three consecutive real editor tasks
with exact Unicode output, one approval each, preserved originals and verified
publication undo. Recorded durations were 42,827 ms, 20,634 ms and 16,465 ms;
these are single-machine observations, not a latency guarantee.

The fourth task saved the requested edit in its task folder but failed normal
shutdown verification: `Editor cleanup is not yet quiescent (application
running: true, extension running: false)`. The task correctly stopped after
five of eight steps, required recovery review and did not publish output.
Independent checks confirm all four source files still match their expected
original bytes, the fourth staged document contains the requested edit and
no fourth publication exists. The fifth editor task and the Excel phase were
not reached. No failed task was retried or counted as a successful run.

The task-owned Code logs show that all four instances used their own
`editor-shared` storage and that the fourth extension host exited with code 0.
Thus the prior shared-storage isolation change is present, but it does not
establish reliable application-process shutdown. The remaining shutdown cause
is not diagnosed by these logs. No deadline or completion assertion was relaxed,
and no unrelated application was terminated. Staging is preserved for review.

## Model correctness failure

The fresh packaged-runtime test successfully loaded the pinned model inside
AppContainer using only the runtime outside the checkout. Its answer to the
unchanged duplicate-file question was: "A duplicate file is another version of
the same file with slightly different contents." The existing content-identity
assertion correctly failed. The test ended before its subsequent tool-request
check, so that part is not counted as exercised in this run.

This is a model correctness failure, not a load failure. The system prompt
already states the correct definition; that instruction did not ensure a
correct answer. Free-form generation currently uses temperature 0.4. This is a
possible source of variability, not a proven diagnosis or a guarantee that
lowering temperature would fix correctness. No prompt, model, assertion or
accepted agent-module hash was changed to turn this failed case into a pass.
The isolated-model acceptance gate remains failed. Native authority does not
depend on accepting this answer as permission.

## Environment and delivery limits

Read-only installed executable metadata reports VS Code 1.140.0 and Excel
16.0.20430.20182. Both native Excel qualification checks currently require
16.0.20430.20146. The newer installed Excel is outside the supported version;
this must be qualified through actual application and failure acceptance before
expanding that boundary. No host application was downgraded or updated.

No available local VM command or running VM was identified by the bounded
discovery. This does not rule out a separately accessible remote test machine.
No installer was executed on the user's normal Windows installation. An isolated
application profile is not a disposable Windows installation.

The available Python interpreter is 3.11.9, while `.python-version` requests
3.12. The passing Python results therefore do not qualify the pinned 3.12
environment. Rust 1.97.0 and Node 24 match the configured toolchain families.

Dependency vulnerability feeds were not refreshed by these offline checks.
The secret scanner and passing regression tests do not establish a complete
security audit, code signing or universal safety.

The current desktop SHA-256 is
`d2ee6419300691a313ec96eb530a07251733491498455d0c5bf421b1328e2971`.
The matching worker SHA-256 is
`991f0f5713f4ca68e87a11fb35d6032ba2049732a6d41fa787014f3299d7ebdc`.
Both are PE64 with ASLR and DEP. The checked Control Flow Guard flag and
certificate table are absent; no signing or CFG guarantee is claimed. Specific
test-hook strings checked by the delivery helper are absent. See
`native-artifacts.json` for exact observations. The candidate outside the
checkout is under the writable artifact root's `audit-1019/bundle/` directory;
it is a verification candidate, not an accepted installation.

## Required repair and acceptance order

First diagnose the editor application shutdown and pass the unchanged five-run
application gate. Keep source preservation, normal-close verification, staged
recovery and publication checks intact. The failure must not be fixed by
ignoring the live process, extending a deadline without qualification or
terminating unrelated work.

Repair and evaluate free-form factual reliability against frozen varied cases;
do not hard-code the failed question or count a lucky rerun as a fix. Qualify
the installed Excel patch through native, failure and optimized desktop tests
before expanding its version boundary. These are unresolved product gates,
not failures of compilation.

Then complete the missing Stage 5 routes/recovery provider and Stage 6 content,
format/media and multi-app implementation, with their own acceptance cases.
Use the pinned Python environment and an actual disposable Windows environment
for the remaining applicable checks. Installation/upgrade/uninstall and
independent whole-product acceptance remain separate. No new installer was
accepted by this audit. Passing aggregate counts do not close these gates.
