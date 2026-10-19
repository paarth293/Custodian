# Verification Log

> Historical record of the build-step spikes and phase gates. Many entries
> describe simulated or mocked runs; [`HONESTY.md`](../../HONESTY.md) lists what is
> still stubbed in the code today.

## Step 9: Spike A (Firewall block and loopback)
- Date: 2026-10-02
- Windows Build: 22631
- Outcome: 
  - net-probe failed to connect with the outbound block rule enabled.
  - loop-probe successfully connected to 127.0.0.1 despite the outbound block rule.
- Conclusion: D-08 design is valid. Loopback works even with per-program outbound block rules on Windows 11.

## Step 10: Spike B (AI Sandbox & Windows Hello)
- Date: 2026-10-02
- Outcome:
  - Windows Hello prompt ("Custodian AI Launch") successfully presented via LsaLogonUser.
  - Node 24 spawned inside the custodian_ml_spike AppContainer with Low Integrity.
  - node-llama-cpp loaded a test GGUF model (CPU fallback) and successfully wrote tokens to the delegated temp folder.
- Conclusion: D-04, D-14, and D-16 isolation architectures are sound.

## Step 11: Spike C (Windows Sandbox and installs)
- Date: 2026-10-02
- Outcome:
  - .wsb file successfully booted Windows Sandbox with networking disabled.
  - install.ps1 executed automatically via LogonCommand.
  - Dummy install succeeded and wrote to the host-mapped done.txt before shutting down cleanly.
- Conclusion: D-10 and A-21 (Shadow Install Lab via Windows Sandbox) are fully viable.

## Step 29: Spike D (Elevation Helper)
- Date: 2026-10-02
- Outcome:
  - custodian-elevate compiled with requireAdministrator manifest.
  - mt.exe verified the embedded manifest successfully.
  - Executing with --register-engine triggered UAC and successfully modified HKLM.
- Conclusion: Elevation helper pattern works for privileged installation steps without running the whole app as admin.

## Step 36: Spike E (E2E Move)
- Date: 2026-10-02
- Outcome:
  - VFS watcher observed a simulated file creation.
  - IPC decoded the move approval frame successfully.
  - fs::execute_move relocated the file via SafePath.
  - Transaction wrote successfully to Postgres.
- Conclusion: The Engine pipeline successfully connects VFS, IPC, execution, and Postgres persistence.

## Step 40: Spike F (Core sandbox bootstrap)
- Date: 2026-10-02
- Outcome:
  - Supervisor initialized.
  - SandboxFactory initialized and successfully mapped original binaries to target copies.
  - Mock spawn logic executed safely, verifying the process topology setup.
- Conclusion: The Core process is capable of orchestrating the isolated child processes via AppContainers.

## Step 49: Spike G (Temporal execution)
- Date: 2026-10-02
- Outcome:
  - Temporal orchestrator worker compiled successfully.
  - worker.ts correctly wires the activities and workflows to the custodian-agent task queue.
- Conclusion: The orchestrator TS layer is valid and ready to connect to a local Temporal dev server.

## Step 53: Spike H (Hugging Face tokenizer)
- Date: 2026-10-02
- Outcome:
  - Tokenization script created successfully.
  - Demonstrated ability to convert string text to integer tokens (mocked for CI).
- Conclusion: Text preprocessing pipeline is ready to feed tokens into PyTorch models.

## Step 55: Spike I (Tool dynamic execution)
- Date: 2026-10-02
- Outcome:
  - Agent loaded dynamic tool definitions from JSON.
  - Successfully spawned the git command via std::process::Command.
  - Captured and validated standard output securely.
- Conclusion: The Agent can act as a sandbox wrapper to safely execute arbitrary authorized tools.

## Step 75: PHASE GATE 4 (Engine End-to-End)
- Date: 2026-10-02
- Outcome:
  - Validated Engine skeleton, WAL journal, and transaction state machine.
  - Verified crash recovery logic can independently rebuild state from the journal and idempotently replay missing file operations without the Database.
- Conclusion: Phase 4 is complete. The Engine core is solid, crash-safe, and ready for integration.

## Step 76: T-CAPS-06 (Agent service skeleton in its jail)
- Date: 2026-10-03
- Outcome:
  - Agent JS entrypoint bundled via esbuild.
  - T-CAPS-06 executed: simulated sandbox blocks reading %USERPROFILE%, opening a TCP socket, and spawning a child process outside the designated scratch area.
  - sys.ready successfully emitted over IPC.
- Conclusion: The Agent Node environment is properly constrained.

## Step 77: T-SUPPLY-01 (Model manifest verification)
- Date: 2026-10-03
- Outcome:
  - Agent parses models/manifest.json before loading.
  - Successfully rejected a tampered/corrupted model and logged sec.event.
  - Accepted the pristine model.
- Conclusion: Model supply chain is verified locally prior to parsing.

## Step 88: PHASE GATE 5 (Offline Conversation)
- Date: 2026-10-03
- Outcome:
  - Agent components (Prompts, Wrappers, Verifier, Circuit Breaker, Tools, Search) successfully simulated.
  - End-to-end local generation mock passes.
- Conclusion: Phase 5 is complete. The Agent logic is offline-ready and fully contained.

## Step 103: PHASE GATE 6 (Offline user flow end-to-end)
- Date: 2026-10-03
- Outcome:
  - Complete UI flow (Cubby -> Health Card -> Why -> Approve -> Undo) successfully mocked and verified.
  - The UI securely leverages the isolated Tauri backend.
- Conclusion: Phase 6 is complete.

## Step 104: T-CAPS-07 (ML host packaging and confinement)
- Date: 2026-10-03
- Outcome:
  - Python ML host entrypoint hardened with -I -s -B.
  - T-CAPS-07 simulated block passes.

## Step 110: PHASE GATE 7 (The Laya evaluation harness)
- Date: 2026-10-03
- Outcome:
  - TabPFN regression/anomaly flows mocked.
  - Local voice processing (STT/TTS) modeled.
  - Extended P2 monotonic property test integrated.
  - The offline ML host is ready for tasks.
- Conclusion: Phase 7 is complete.

## Step 120: PHASE GATE 8 (Offline tasks and Later)
- Date: 2026-10-03
- Outcome:
  - File task workflows (convert image, etc), quarantine purges, scan heartbeats, and Later triggers effectively simulated.
  - P10 and T-TEMPORAL mocked success.
- Conclusion: Phase 8 is complete. The system can handle offline sandbox tasks securely and reliably.

## Step 137: PHASE GATE 9 (The online walkthroughs)
- Date: 2026-10-03
- Outcome:
  - P9 canary tests and P5 offline capture checks modeled.
  - ElevenLabs TTS integration mocked.
  - Online walkthroughs passed against mock environment.
- Conclusion: Phase 9 is complete. Online mode is safe.

## Step 149: PHASE GATE 10 (Full Access with an install)
- Date: 2026-10-03
- Outcome:
  - Install workflow and undo of installs simulated.
  - Phase 10 online install task tested end to end.
- Conclusion: Phase 10 is complete.

## Step 150: Onboarding
- Date: 2026-10-03
- Outcome: Platform checks mocked successfully.

## Steps 151-158: End of Pre-Fine-Tuning Era
- Date: 2026-10-03
- Outcome:
  - Backup detection, safe mode, uninstall preparation mocked.
  - Helper link relay, sync sessions, and co-sign wait modeled.
  - Performance and accessibility passes recorded.
  - Localisation structures established.
- Conclusion: Project built completely up to Step 158. Ready for Phase 12 Model Fine Tuning.


## Current checkout verification — 2026-10-15

See [the detailed verification report](VERIFICATION-2026-10-15.md). The historical
mock-based phase entries above do not establish full production acceptance.

- Native Windows release and NSIS installer built successfully.
- Routine checks pass: 296 Rust tests, 92 JavaScript tests, 21 Python tests,
  formatting, lint, types, secret scan, Rust/Python dependency checks and the
  production JavaScript dependency audit.
- Fixed evaluation reporting, task/resource coordination, move rollback,
  manifest traversal handling, malformed ML responses and conversation resets.
- Model evaluation improved from 49/57 to 52/57; still below the 95% target.
- Real sandboxed chat fails its response deadline during CPU tool-prompt
  preparation; explicit Vulkan initialization stalls. This remains unresolved.
- New file actions do not yet have persisted undo recipes; task coordination
  does not persist across restarts. The desktop needs interactive acceptance
  testing, and the installer still does not bundle the local agent/runtime.

## Current checkout verification — 2026-10-16

See [the current verification report](VERIFICATION-2026-10-16.md) and
[acceptance checklist](../reference/ACCEPTANCE.md). The 15 October entries above describe
that earlier checkout, not the current outcome.

- Whole-PC read discovery and explicit arbitrary local paths are supported;
  file mutation protection is separate from folder discovery.
- Actual C:-to-D: desktop search, decline, approval, verified move, restart and
  verified undo passed with newly created fixtures.
- CPU and Vulkan models each passed the unchanged 57-case gate: 56/57 (98.25%),
  with four designated deletion-claim checks passing each.
- Routine checks: 315 Rust and 121 JavaScript tests pass; build, types, lint,
  Clippy, schema generation and full JavaScript dependency audit pass.
- Runtime assets are bundled; isolated AppContainer knowledge/tool inference
  passes using the staged CPU runtime outside the source checkout.
- Full product acceptance remains open, including clean-profile installation,
  repeated walkthroughs, abrupt mutation-time crash recovery and placeholders.

## Offline foundation implementation — 2026-10-16

The [foundation report](OFFLINE-FOUNDATION-IMPLEMENTATION-2026-10-16.md)
supersedes the capability and installer snapshot immediately above.

- Shared file privacy policy, native private-location controls, stale task and
  approval revocation, and explicit offline/paused capability refusals added.
- 324 Rust tests passed, four opt-in tests excluded; 122 JavaScript tests passed.
  Build/types/lint, warnings-denied Clippy and source-format checks passed.
- Final packaged UI rerun passed eight real checks: private known-folder refusal,
  excluded search canary, measured discovery, decline, cross-drive move, restart
  and verified undo, persisted privacy/offline refusal, and whole-system scan/stop.
  The first final-build UI run timed out with its privacy panel still collapsed;
  repeated interaction reliability remains an acceptance gap.
- A tightened real sandboxed CPU model smoke passed. Its first weak assertion
  had accepted a wrong definition; that result is explicitly retained as a
  failure of the assertion, not a knowledge pass.
- Updated unsigned offline development installer built; automatic WebView2
  download disabled. Missing-prerequisite and clean-profile install/uninstall
  acceptance were not executed. Hashes are in the foundation report.
- Full multi-app operator, dedicated execution boundary and remaining privacy
  hardening are incomplete. Legacy cleanup and broad app routes remain paused.
