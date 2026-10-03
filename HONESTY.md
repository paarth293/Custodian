# Honesty

## Stubbed
- `crates/custodian-winsec`: AppContainer spawn, job-object limits, Windows Hello
  prompt and DPAPI (`protect_data`/`unprotect_data`) are mocks (the "encryption" is a
  byte reversal). No real Win32 calls are made yet.
- `crates/custodian-engine/tests`: `gate_7`, `gate_9`, `gate_10`, `p2_extended`,
  `p9_p5_canary`, `p10_shadow_real` and `t_elev_install` are `#[ignore]`d placeholders.
  They contain `todo!()` and verify nothing; they must not be counted as passing gates.

## Cut
(Nothing yet)

## Unmeasured
(Nothing yet)

## Not Entered
(Nothing yet)
