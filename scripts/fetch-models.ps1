<#
.SYNOPSIS
    Downloads a GGUF model into models/ and pins its SHA-256 in models/manifest.json.

.EXAMPLE
    ./scripts/fetch-models.ps1 -Repo your-org/laya -HfToken $env:HF_TOKEN

.EXAMPLE
    ./scripts/fetch-models.ps1 -Mock   # placeholder file for CI and UI work
#>
param (
    [string]$Repo,
    [string]$File = 'laya.Q4_K_M.gguf',
    [string]$HfToken = $env:HF_TOKEN,
    [switch]$Mock
)

$ErrorActionPreference = 'Stop'
$modelsDir = Join-Path $PSScriptRoot '..' 'models'
New-Item -ItemType Directory -Path $modelsDir -Force | Out-Null
$target = Join-Path $modelsDir $File

if ($Mock) {
    Set-Content -Path $target -Value 'mock model content' -NoNewline
    Write-Host "Created placeholder $target"
} else {
    if (-not $Repo) { throw '-Repo is required (or use -Mock).' }
    $headers = @{}
    if ($HfToken) { $headers['Authorization'] = "Bearer $HfToken" }
    $url = "https://huggingface.co/$Repo/resolve/main/$File"
    Write-Host "Downloading $url ..."
    Invoke-WebRequest -Uri $url -Headers $headers -OutFile $target
}

$manifestPath = Join-Path $modelsDir 'manifest.json'
$manifest = @{}
if (Test-Path $manifestPath) {
    (Get-Content $manifestPath -Raw | ConvertFrom-Json).PSObject.Properties |
        ForEach-Object { $manifest[$_.Name] = $_.Value }
}
$hash = (Get-FileHash -Algorithm SHA256 -Path $target).Hash.ToLowerInvariant()
$manifest[$File] = @{ sha256 = $hash }
$manifest | ConvertTo-Json -Depth 3 | Set-Content -Path $manifestPath
Write-Host "Pinned $File ($hash) in $manifestPath"
