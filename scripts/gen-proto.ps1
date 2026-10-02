Write-Host "Generating Rust types..."
npx quicktype -s schema proto/core/Transaction.json -o crates/custodian-proto/src/lib.rs --visibility public

Write-Host "Generating TS types..."
New-Item -ItemType Directory -Path "services/agent/src" -Force | Out-Null
npx quicktype -s schema proto/core/Transaction.json -o services/agent/src/proto.ts --just-types

Write-Host "Generating Python types..."
npx quicktype -s schema proto/core/Transaction.json -o services/mlhost/custodian_proto.py
