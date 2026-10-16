# Regenerates the shared wire types from proto/core/Transaction.json.
# Review the generated diff: hand-written docs and derives may need re-applying.
$ErrorActionPreference = 'Stop'
node (Join-Path $PSScriptRoot 'gen-proto.mjs') @args
exit $LASTEXITCODE
