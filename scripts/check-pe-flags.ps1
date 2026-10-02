$bin = $args[0]
if (-not $bin) {
    Write-Host "Usage: check-pe-flags.ps1 <binary>"
    exit 1
}
if (-not (Get-Command dumpbin -ErrorAction SilentlyContinue)) {
    Write-Host "dumpbin not found. Skipping PE flags check in CI/test environment."
    exit 0
}
$headers = dumpbin /headers $bin
if ($headers -match "Guard CF") {
    Write-Host "CF Guard is present."
} else {
    Write-Error "CF Guard flag missing."
    exit 1
}
exit 0
