$expectedDirs = @(
    "proto",
    "crates",
    "crates/custodian-winsec",
    "crates/custodian-ipc",
    "crates/custodian-proto",
    "crates/custodian-store",
    "crates/custodian-kernel",
    "crates/custodian-scan",
    "crates/custodian-evidence",
    "crates/custodian-analyze",
    "crates/custodian-readapi",
    "crates/custodian-journal",
    "crates/custodian-ledger",
    "crates/custodian-digest",
    "crates/custodian-engine",
    "crates/custodian-sandbox",
    "crates/custodian-installlab",
    "crates/custodian-elevate",
    "crates/custodian-parserhost",
    "crates/custodian-fixtures",
    "crates/custodian-testkit",
    "apps",
    "apps/desktop",
    "apps/helper-pwa",
    "services",
    "services/agent",
    "services/orchestrator",
    "services/broker",
    "services/relay",
    "services/mlhost",
    "training",
    "tools",
    "models",
    "installer",
    "tests",
    "tests/e2e",
    "tests/security",
    "tests/fixtures",
    "docs",
    ".github",
    ".github/workflows",
    "scripts"
)

$missing = 0
foreach ($dir in $expectedDirs) {
    if (-not (Test-Path $dir)) {
        Write-Error "Missing directory: $dir"
        $missing++
    }
}

if ($missing -gt 0) {
    exit 1
} else {
    Write-Host "Layout is correct."
    exit 0
}
