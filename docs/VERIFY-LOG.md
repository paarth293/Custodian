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
