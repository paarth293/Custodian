# Fails if a Windows binary was built without Control Flow Guard.
# Usage: ./scripts/check-pe-flags.ps1 <path-to-binary>
param ([Parameter(Mandatory = $true)][string]$Binary)

if (-not (Get-Command dumpbin -ErrorAction SilentlyContinue)) {
    Write-Warning 'dumpbin not found (run from a VS Developer shell); skipping PE flag check.'
    exit 0
}

$headers = dumpbin /headers $Binary
if ($headers -match 'Guard CF') {
    Write-Host "CF Guard is present in $Binary."
    exit 0
}
Write-Error "CF Guard flag missing from $Binary."
exit 1
