param([string]$OutputPath = 'docs/EVIDENCE/stage-4/app-environment.json')
$ErrorActionPreference = 'Stop'
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$target = [IO.Path]::GetFullPath((Join-Path $repoRoot $OutputPath))
$evidenceRoot = [IO.Path]::GetFullPath((Join-Path $repoRoot 'docs/EVIDENCE/stage-4'))
if (!$target.StartsWith($evidenceRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Probe output must be a new file inside docs/EVIDENCE/stage-4.'
}
if (Test-Path -LiteralPath $target) { throw 'Refusing to replace existing probe evidence.' }

# Fixed installed-program metadata only. No app launch, document/window/title
# enumeration, UI Automation, file-content extraction or personal-directory scan.
$candidates = [Collections.Generic.List[object]]::new()
function Add-Candidate([string]$Role, [string]$Kind, [string]$Path) {
    if (!$Path -or !(Test-Path -LiteralPath $Path -PathType Leaf)) { return }
    $version = [Diagnostics.FileVersionInfo]::GetVersionInfo($Path)
    $candidates.Add([pscustomobject]@{
        role = $Role; kind = $Kind; path = $Path
        fileVersion = $version.FileVersion; productVersion = $version.ProductVersion
        adapterQualified = $false
    })
}
Add-Candidate 'editor' 'Windows launcher' (Join-Path $env:windir 'System32/notepad.exe')
$packageProbe = 'unavailable'
if (Get-Command Get-AppxPackage -ErrorAction SilentlyContinue) {
    try {
        foreach ($package in @(Get-AppxPackage -Name Microsoft.WindowsNotepad)) {
            Add-Candidate 'editor' 'packaged editor' (Join-Path $package.InstallLocation 'Notepad/Notepad.exe')
        }
        $packageProbe = 'completed'
    } catch { $packageProbe = 'unavailable; installed editor package metadata could not be read' }
}
foreach ($programRoot in @($env:ProgramFiles, ${env:ProgramFiles(x86)})) {
    if (!$programRoot) { continue }
    Add-Candidate 'spreadsheet' 'Office executable' (Join-Path $programRoot 'Microsoft Office/root/Office16/EXCEL.EXE')
}
$report = [ordered]@{
    schemaVersion = 1; observedAtUtc = [DateTime]::UtcNow.ToString('o')
    condition = 'Read-only installed-executable metadata; not app/API/UIA/save qualification'
    packageProbe = $packageProbe; candidates = @($candidates.ToArray())
    applicationsLaunched = 0; windowTitlesObserved = 0; documentContentsObserved = 0
    editorAdapterAccepted = $false; spreadsheetAdapterAccepted = $false
    nextGate = 'Qualify isolated document identity and pre-observation privacy before app content access'
}
[IO.Directory]::CreateDirectory($evidenceRoot) | Out-Null
$bytes = [Text.UTF8Encoding]::new($false).GetBytes(($report | ConvertTo-Json -Depth 6) + "`n")
$stream = [IO.File]::Open($target, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
try { $stream.Write($bytes, 0, $bytes.Length) } finally { $stream.Dispose() }
Write-Output "Recorded $($candidates.Count) installed executable candidates; no application/document observation."
