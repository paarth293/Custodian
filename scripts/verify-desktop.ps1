param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [Parameter(Mandatory = $true)][string]$FixtureRoot,
    [string]$DestinationRoot,
    [switch]$CheckWholeSystemRead,
    [switch]$CheckPrivacy
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms

# Never reuse or overwrite an existing directory, including a real user folder.
if (Test-Path -LiteralPath $FixtureRoot) { throw 'Use a new, empty fixture location.' }
$fixture = New-Item -ItemType Directory -Path $FixtureRoot
Set-Content -LiteralPath (Join-Path $fixture.FullName '.custodian-fixture-root') -Value ''
foreach ($label in @('Downloads', 'Documents', 'Desktop', 'Pictures', 'Videos', 'Music')) {
    New-Item -ItemType Directory -Path (Join-Path $fixture.FullName "folders\$label") | Out-Null
}
$sourceFolder = Join-Path $fixture.FullName 'custom-source'
New-Item -ItemType Directory -Path $sourceFolder | Out-Null
if (!$DestinationRoot) { $DestinationRoot = Join-Path $fixture.FullName 'WorkProjects' }
if (Test-Path -LiteralPath $DestinationRoot) { throw 'Use a new destination fixture location.' }
New-Item -ItemType Directory -Path $DestinationRoot | Out-Null
Set-Content -LiteralPath (Join-Path $DestinationRoot '.custodian-fixture-root') -Value ''
$source = Join-Path $sourceFolder 'acceptance-largest.txt'
$destination = Join-Path $DestinationRoot 'acceptance-largest.txt'
[IO.File]::WriteAllBytes($source, [byte[]](1..128 * 64))
[IO.File]::WriteAllText((Join-Path $sourceFolder 'small.txt'), 'small')
$expectedHash = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
$privateFolder = Join-Path $sourceFolder 'private'
$privateCanary = Join-Path $privateFolder 'never-observe-canary.txt'
if ($CheckPrivacy) {
    New-Item -ItemType Directory -Path $privateFolder | Out-Null
    [IO.File]::WriteAllBytes($privateCanary, [byte[]]::new(16384))
    $expectedCanaryHash = (Get-FileHash -LiteralPath $privateCanary -Algorithm SHA256).Hash
}
$priorRoot = $env:CUSTODIAN_VERIFICATION_ROOT
$priorGpu = $env:CUSTODIAN_LLAMA_GPU
$env:CUSTODIAN_VERIFICATION_ROOT = $fixture.FullName
$env:CUSTODIAN_LLAMA_GPU = 'off'
$script:desktop = $null
$steps = [System.Collections.Generic.List[string]]::new()

function Wait-For([scriptblock]$Condition, [string]$Description, [int]$Seconds = 30) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Seconds)
    do {
        $result = & $Condition
        if ($result) { return $result }
        [Threading.Thread]::Sleep(100)
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timed out: $Description"
}

function Root-Element {
    $script:desktop.Refresh()
    if ($script:desktop.HasExited) { throw 'The acceptance desktop exited unexpectedly.' }
    if ($script:desktop.MainWindowHandle -eq [IntPtr]::Zero) { return $null }
    return [Windows.Automation.AutomationElement]::FromHandle($script:desktop.MainWindowHandle)
}

function Find-Control([string]$Name, [Windows.Automation.ControlType]$Type) {
    $rootElement = Root-Element
    if (!$rootElement) { return $null }
    $condition = [Windows.Automation.AndCondition]::new(
        [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty, $Name),
        [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, $Type)
    )
    return $rootElement.FindFirst([Windows.Automation.TreeScope]::Descendants, $condition)
}

function Click-Control([string]$Name) {
    $control = Wait-For {
        $candidate = Find-Control $Name ([Windows.Automation.ControlType]::Button)
        if ($candidate -and $candidate.Current.IsEnabled) { $candidate }
    } "enabled button $Name"
    $pattern = $null
    if ($control.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
        $pattern.Invoke()
    } else {
        $control.SetFocus()
        [Windows.Forms.SendKeys]::SendWait('{ENTER}')
    }
}

function Read-DesktopText {
    $rootElement = Root-Element
    if (!$rootElement) { return '' }
    $elements = $rootElement.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
    return (($elements | ForEach-Object { $_.Current.Name }) -join "`n")
}

function Send-Message([string]$Text) {
    $inputControl = Wait-For { Find-Control 'Chat input' ([Windows.Automation.ControlType]::Edit) } 'chat input'
    $value = $inputControl.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)
    $value.SetValue($Text)
    Wait-For {
        $current = Find-Control 'Chat input' ([Windows.Automation.ControlType]::Edit)
        $currentValue = $current.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)
        $currentValue.Current.Value -eq $Text
    } 'complete chat input' | Out-Null
    Click-Control 'Send'
}

function Start-Desktop([string]$Suffix) {
    $script:desktop = Start-Process -FilePath $Executable -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput (Join-Path $fixture.FullName "$Suffix.stdout.log") `
        -RedirectStandardError (Join-Path $fixture.FullName "$Suffix.stderr.log")
    Wait-For { Root-Element } 'desktop window' | Out-Null
    Click-Control 'Cubby. Click to talk, drag to move, right-click for options.'
    Wait-For { Find-Control 'Chat input' ([Windows.Automation.ControlType]::Edit) } 'opened chat' | Out-Null
}

function Assert-Original {
    if (!(Test-Path -LiteralPath $source) -or (Test-Path -LiteralPath $destination)) { throw 'Fixture moved before approval, or undo failed.' }
    if ((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -ne $expectedHash) { throw 'Original payload changed.' }
}

function Stop-Desktop {
    # The browser can briefly outlive its host. Wait for those exact child
    # handles before reopening the same profile, rather than touching other apps.
    $children = @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $($script:desktop.Id)" | ForEach-Object {
        try { [Diagnostics.Process]::GetProcessById($_.ProcessId) } catch { }
    })
    $script:desktop.Kill($true)
    $script:desktop.WaitForExit()
    foreach ($child in $children) {
        if (!$child.WaitForExit(15000)) { throw 'An acceptance browser child did not exit.' }
        $child.Dispose()
    }
}

try {
    Start-Desktop 'first'
    if ($CheckPrivacy) {
        Click-Control 'Privacy and capabilities'
        $privateInput = Wait-For {
            $candidate = Find-Control 'Private location path' ([Windows.Automation.ControlType]::Edit)
            if ($candidate -and $candidate.Current.IsEnabled) { $candidate }
        } 'loaded privacy input'
        $privateValue = $privateInput.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)
        $privateValue.SetValue($privateFolder)
        Click-Control 'Add location'
        $documentsInput = Wait-For { Find-Control 'Private location path' ([Windows.Automation.ControlType]::Edit) } 'second privacy input'
        $documentsValue = $documentsInput.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)
        $documentsValue.SetValue((Join-Path $fixture.FullName 'folders\Documents'))
        Click-Control 'Add location'
        Click-Control 'Save privacy rules'
        Wait-For { (Read-DesktopText) -match 'Saved. Previous tasks' } 'persisted privacy rules' | Out-Null
        Click-Control 'Privacy and capabilities'
        Send-Message 'Open my Documents folder'
        Wait-For { (Read-DesktopText) -match 'Access is blocked by your privacy rules' } 'private folder opening refusal' | Out-Null
        $steps.Add('Opening a private known folder was refused before handing it to Explorer.')
    }
    Send-Message ('Find the largest file in "' + $sourceFolder + '"')
    Wait-For { (Read-DesktopText) -match '8192 bytes' } 'measured largest-file result' | Out-Null
    if ($CheckPrivacy) {
        if ((Read-DesktopText) -match 'never-observe-canary') { throw 'Private canary leaked into search results.' }
        $steps.Add('Native privacy controls excluded the larger private canary from the actual desktop search.')
    }
    $steps.Add('Live search displayed the measured 8192-byte fixture file.')
    Send-Message ('Move it to "' + $DestinationRoot + '"')
    Wait-For { (Read-DesktopText) -match 'Destination:' } 'exact-path approval card' | Out-Null
    Assert-Original
    Click-Control 'Not now'
    Wait-For { (Read-DesktopText) -match "won.t move" } 'declined action result' | Out-Null
    Assert-Original
    $steps.Add('Declining the live approval left both locations unchanged.')
    Send-Message ('Move it to "' + $DestinationRoot + '"')
    Wait-For { Find-Control 'Move' ([Windows.Automation.ControlType]::Button) } 'new move approval' | Out-Null
    Assert-Original
    Click-Control 'Move'
    Wait-For { (Read-DesktopText) -match 'undo record was saved' } 'verified completion' | Out-Null
    if ((Test-Path -LiteralPath $source) -or !(Test-Path -LiteralPath $destination)) { throw 'Move did not execute as reported.' }
    if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $expectedHash) { throw 'Moved payload differs.' }
    $steps.Add('Live approval executed a verified move with matching bytes.')
    Stop-Desktop
    Start-Desktop 'restarted'
    Send-Message 'undo'
    Wait-For { (Read-DesktopText) -match 'Undid the verified file task' } 'undo after desktop restart' | Out-Null
    Assert-Original
    $steps.Add('Restarted desktop reopened the undo record and restored identical bytes.')
    if ($CheckPrivacy) {
        Send-Message ('Find the largest file in "' + $sourceFolder + '"')
        Wait-For { (Read-DesktopText) -match '8192 bytes' } 'privacy-protected search after restart' | Out-Null
        if ((Read-DesktopText) -match 'never-observe-canary') { throw 'Private canary leaked after restart.' }
        Send-Message 'Search the web for Windows tips'
        Wait-For { (Read-DesktopText) -match 'Online actions are unavailable' } 'native offline handoff refusal' | Out-Null
        if ((Get-FileHash -LiteralPath $privateCanary -Algorithm SHA256).Hash -ne $expectedCanaryHash) { throw 'Private fixture changed.' }
        $steps.Add('Private-zone rules survived restart and the desktop refused an online handoff.')
    }
    if ($CheckWholeSystemRead) {
        Send-Message 'Find the largest file across all drives'
        Wait-For { (Read-DesktopText) -match '(?s)Measuring largest files in .*C:.*D:' } 'whole-system scope in the task panel' | Out-Null
        Send-Message 'How much free space?'
        Wait-For { (Read-DesktopText) -match "Here.s your storage" } 'independent answer during whole-system scan' | Out-Null
        Click-Control 'Stop current work'
        Wait-For { (Read-DesktopText) -match 'Okay, I stopped' } 'cancelled whole-system scan' 10 | Out-Null
        Assert-Original
        $steps.Add('Whole-system read included C: and D:, answered an independent question, and stopped without changing the fixture.')
    }
    $steps | ForEach-Object { Write-Output "PASS: $_" }
    Write-Output "Fixture preserved at $($fixture.FullName)"
} catch {
    try { Read-DesktopText | Set-Content -LiteralPath (Join-Path $fixture.FullName 'failure-ui.txt') } catch { }
    throw
} finally {
    if ($script:desktop -and !$script:desktop.HasExited) {
        Stop-Desktop
    }
    $env:CUSTODIAN_VERIFICATION_ROOT = $priorRoot
    $env:CUSTODIAN_LLAMA_GPU = $priorGpu
}
