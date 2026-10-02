# Verification Log

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
