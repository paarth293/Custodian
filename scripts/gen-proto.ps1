# Regenerates the shared wire types from proto/core/Transaction.json.
# Review the generated diff: hand-written docs and derives may need re-applying.
$ErrorActionPreference = 'Stop'
$schema = 'proto/core/Transaction.json'

Write-Host 'Generating Rust types...'
npx quicktype -s schema $schema --top-level Transaction --lang rust --visibility public `
    --derive-debug --derive-clone --derive-partial-eq -o crates/custodian-proto/src/lib.rs

Write-Host 'Generating TypeScript types...'
npx quicktype -s schema $schema --top-level Transaction --lang ts --just-types `
    -o services/agent/src/proto.ts

Write-Host 'Generating Python types...'
npx quicktype -s schema $schema --top-level Transaction --lang python --python-version 3.7 `
    -o services/mlhost/mlhost/proto.py
