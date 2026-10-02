# Verification Log

## Step 9: Spike A (Firewall block and loopback)
- Date: 2026-10-02
- Windows Build: 22631
- Outcome: 
  - net-probe failed to connect with the outbound block rule enabled.
  - loop-probe successfully connected to 127.0.0.1 despite the outbound block rule.
- Conclusion: D-08 design is valid. Loopback works even with per-program outbound block rules on Windows 11.
