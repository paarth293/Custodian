param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [Parameter(Mandatory = $true)][string]$FixtureRoot,
    [string]$DestinationRoot,
    [switch]$CheckWholeSystemRead,
    [switch]$CheckPrivacy,
    [switch]$CheckRoutine,
    [switch]$CheckWorkflow,
    [switch]$CheckAuthority,
    [switch]$CheckCoordinator,
    [switch]$CheckLanguage
)

$ErrorActionPreference = 'Stop'
if ($CheckCoordinator -and !$CheckWorkflow) { throw 'Coordinator verification requires workflow verification.' }
if ($CheckLanguage -and !$CheckWorkflow) { throw 'Language verification requires workflow verification.' }
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms

# Use the already-pinned kernel handle for both termination and exit checks.
# Process-tree helpers can re-enumerate recycled PIDs after the host has died.
if (!('CustodianFixtureProcess' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
public static class CustodianFixtureProcess {
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern uint WaitForSingleObject(SafeProcessHandle process, uint milliseconds);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool TerminateProcess(SafeProcessHandle process, uint code);
    public static bool Exited(SafeProcessHandle process, uint milliseconds) {
        uint result = WaitForSingleObject(process, milliseconds);
        if (result == 0) return true;
        if (result == 258) return false;
        throw new Win32Exception(Marshal.GetLastWin32Error());
    }
    public static void Terminate(SafeProcessHandle process) {
        if (!Exited(process, 0) && !TerminateProcess(process, 1) && !Exited(process, 0))
            throw new Win32Exception(Marshal.GetLastWin32Error());
    }
}
'@
}

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
if ($CheckWorkflow) { [IO.File]::WriteAllBytes((Join-Path $sourceFolder 'acceptance-photo.jpg'), [byte[]]::new(1024)) }
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
        try { $result = & $Condition } catch [Windows.Automation.ElementNotAvailableException] { $result = $null }
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
    $action = Wait-For {
        $candidate = Find-Control $Name ([Windows.Automation.ControlType]::Button)
        $pattern = $null
        if ($candidate -and $candidate.Current.IsEnabled) {
            if ($candidate.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
                [pscustomobject]@{ Kind = 'Invoke'; Pattern = $pattern }
            } elseif ($candidate.TryGetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern, [ref]$pattern)) {
                [pscustomobject]@{ Kind = 'ExpandCollapse'; Pattern = $pattern }
            }
        }
    } "actionable button $Name"
    if ($action.Kind -eq 'Invoke') {
        [IO.File]::AppendAllText((Join-Path $fixture.FullName 'actions.txt'), "$Name : Invoke`n")
        $action.Pattern.Invoke()
    } else {
        [IO.File]::AppendAllText((Join-Path $fixture.FullName 'actions.txt'), "$Name : ExpandCollapse`n")
        if ($action.Pattern.Current.ExpandCollapseState -eq [Windows.Automation.ExpandCollapseState]::Expanded) {
            $action.Pattern.Collapse()
        } else { $action.Pattern.Expand() }
    }
}

function Read-DesktopText {
    $rootElement = Root-Element
    if (!$rootElement) { return '' }
    $elements = $rootElement.FindAll([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.Condition]::TrueCondition)
    return (($elements | ForEach-Object { $_.Current.Name }) -join "`n")
}

function Open-RoutineControls {
    if (!(Find-Control 'Enable automatic verified backup' ([Windows.Automation.ControlType]::CheckBox))) {
        $button = Find-Control 'Routine permissions' ([Windows.Automation.ControlType]::Button)
        $pattern = $null
        if ($button -and $button.TryGetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern, [ref]$pattern)) {
            [IO.File]::AppendAllText((Join-Path $fixture.FullName 'actions.txt'), "Routine permissions : Expand`n")
            $pattern.Expand()
        } else { Click-Control 'Routine permissions' }
    }
    Wait-For { Find-Control 'Enable automatic verified backup' ([Windows.Automation.ControlType]::CheckBox) } 'expanded routine controls' | Out-Null
}

function Close-RoutineControls {
    if (Find-Control 'Enable automatic verified backup' ([Windows.Automation.ControlType]::CheckBox)) {
        $button = Find-Control 'Routine permissions' ([Windows.Automation.ControlType]::Button)
        $pattern = $null
        if ($button -and $button.TryGetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern, [ref]$pattern)) {
            [IO.File]::AppendAllText((Join-Path $fixture.FullName 'actions.txt'), "Routine permissions : Collapse`n")
            $pattern.Collapse()
        } else { Click-Control 'Routine permissions' }
    }
    Wait-For { !(Find-Control 'Enable automatic verified backup' ([Windows.Automation.ControlType]::CheckBox)) } 'collapsed routine controls' | Out-Null
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
    # Allow native window resizing/hit testing to settle before another UIA invoke.
    [Threading.Thread]::Sleep(350)
}

function Assert-Original {
    if (!(Test-Path -LiteralPath $source) -or (Test-Path -LiteralPath $destination)) { throw 'Fixture moved before approval, or undo failed.' }
    if ((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -ne $expectedHash) { throw 'Original payload changed.' }
}

function Stop-Desktop {
    # Drop retained accessibility proxies while their provider is still alive.
    # These are the only top-level UIA values retained by this fixture script.
    foreach ($proxyName in @('privateInput', 'privateValue', 'documentsInput', 'documentsValue')) {
        Set-Variable -Scope Script -Name $proxyName -Value $null
    }
    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()
    [GC]::Collect()
    $rootHandle = $script:desktop.SafeHandle
    $rootBirth = $script:desktop.StartTime.ToUniversalTime()
    $known = [Collections.Generic.Dictionary[uint32, datetime]]::new()
    $known[[uint32]$script:desktop.Id] = $rootBirth
    $depths = [Collections.Generic.Dictionary[uint32, int]]::new()
    $depths[[uint32]$script:desktop.Id] = 0
    $children = [Collections.Generic.List[object]]::new()
    $snapshot = @(Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, CreationDate, Name)
    do {
        $added = $false
        foreach ($record in $snapshot) {
            $processId = [uint32]$record.ProcessId
            $parentId = [uint32]$record.ParentProcessId
            if ($known.ContainsKey($processId) -or !$known.ContainsKey($parentId)) { continue }
            $birth = $record.CreationDate.ToUniversalTime()
            # A surviving orphan with a recycled parent PID is not our child.
            if ($birth -lt $known[$parentId] -or $birth -lt $rootBirth) { continue }
            $capturedChild = $null
            try {
                $capturedChild = [Diagnostics.Process]::GetProcessById($processId)
                $handle = $capturedChild.SafeHandle
                if ([Math]::Abs(($capturedChild.StartTime.ToUniversalTime() - $birth).TotalMilliseconds) -gt 1) {
                    $capturedChild.Dispose()
                    continue
                }
                $known[$processId] = $birth
                $depths[$processId] = $depths[$parentId] + 1
                $children.Add([pscustomobject]@{ Process = $capturedChild; Handle = $handle; Id = $processId; Depth = $depths[$processId] })
                [IO.File]::AppendAllText((Join-Path $fixture.FullName 'actions.txt'), "Captured test descendant $processId $($record.Name) $($birth.ToString('o'))`n")
                $added = $true
            } catch { if ($capturedChild) { $capturedChild.Dispose() } }
        }
    } while ($added)
    $cleanupErrors = [Collections.Generic.List[string]]::new()
    try {
        # Terminate leaves before their browser/host parents. Keep the pinned
        # handles, identity checks and mandatory exit checks; never kill by name.
        foreach ($child in ($children | Sort-Object Depth -Descending)) {
            try { [CustodianFixtureProcess]::Terminate($child.Handle) }
            catch { $cleanupErrors.Add("Could not terminate captured descendant $($child.Id): $_") }
        }
        [CustodianFixtureProcess]::Terminate($rootHandle)
        if (![CustodianFixtureProcess]::Exited($rootHandle, 15000)) { $cleanupErrors.Add('The fixture host did not exit.') }
        foreach ($child in $children) {
            if (![CustodianFixtureProcess]::Exited($child.Handle, 30000)) {
                $cleanupErrors.Add("Captured descendant $($child.Id) did not exit after test cleanup.")
            } else {
                [IO.File]::AppendAllText((Join-Path $fixture.FullName 'actions.txt'), "Exited test descendant $($child.Id)`n")
            }
        }
    } finally {
        foreach ($child in $children) { $child.Process.Dispose() }
    }
    if ($cleanupErrors.Count -gt 0) { throw ($cleanupErrors -join "`n") }
}

try {
    Start-Desktop 'first'
    if ($CheckAuthority) {
        $duplicate = Start-Process -FilePath $Executable -WindowStyle Hidden -PassThru `
            -RedirectStandardOutput (Join-Path $fixture.FullName 'duplicate.stdout.log') `
            -RedirectStandardError (Join-Path $fixture.FullName 'duplicate.stderr.log')
        try {
            if (!$duplicate.WaitForExit(15000)) { throw 'A second authoritative desktop instance remained running.' }
            if ((Get-Content -Raw -LiteralPath (Join-Path $fixture.FullName 'duplicate.stderr.log')) -notmatch 'already running for this user') {
                throw 'Second instance did not report the host authority lock.'
            }
            Send-Message 'How much free space?'
            Wait-For { (Read-DesktopText) -match "Here.s your storage" } 'original host remains responsive after duplicate refusal' | Out-Null
            $steps.Add('A second desktop exited before loading state; the authoritative host remained responsive.')
        } finally {
            if (!$duplicate.HasExited) { $duplicate.Kill($true); $duplicate.WaitForExit() }
            $duplicate.Dispose()
        }
        $otherProfile = Join-Path $fixture.FullName 'other-profile'
        New-Item -ItemType Directory -Path $otherProfile | Out-Null
        Set-Content -LiteralPath (Join-Path $otherProfile '.custodian-fixture-root') -Value ''
        $env:CUSTODIAN_VERIFICATION_ROOT = $otherProfile
        $otherHost = $null
        try {
            $otherHost = Start-Process -FilePath $Executable -WindowStyle Hidden -PassThru `
                -RedirectStandardOutput (Join-Path $fixture.FullName 'other-profile.stdout.log') `
                -RedirectStandardError (Join-Path $fixture.FullName 'other-profile.stderr.log')
            if (!$otherHost.WaitForExit(15000)) { throw 'An alternate profile bypassed the user authority lease.' }
            if ((Get-Content -Raw -LiteralPath (Join-Path $fixture.FullName 'other-profile.stderr.log')) -notmatch 'already running for this user') {
                throw 'Alternate profile did not report the shared authority lock.'
            }
            if (Test-Path -LiteralPath (Join-Path $otherProfile 'app-data/background-tasks.dpapi')) { throw 'Alternate host loaded mutable task state.' }
            $steps.Add('An alternate verification profile also exited before loading task state; profile overrides did not bypass the user authority lease.')
        } finally {
            $env:CUSTODIAN_VERIFICATION_ROOT = $fixture.FullName
            if ($otherHost) {
                if (!$otherHost.HasExited) { $otherHost.Kill($true); $otherHost.WaitForExit() }
                $otherHost.Dispose()
            }
        }
    }
    if ($CheckWorkflow) {
        for ($cycle = 0; $cycle -lt 5; $cycle++) {
            Click-Control 'Cubby. Click to talk, drag to move, right-click for options.'
            Wait-For { !(Find-Control 'Chat input' ([Windows.Automation.ControlType]::Edit)) } 'closed chat through accessibility activation' | Out-Null
            Click-Control 'Cubby. Click to talk, drag to move, right-click for options.'
            Wait-For { Find-Control 'Chat input' ([Windows.Automation.ControlType]::Edit) } 'reopened chat through accessibility activation' | Out-Null
            [Threading.Thread]::Sleep(350)
            Open-RoutineControls
            Close-RoutineControls
        }
        $steps.Add('Five repeated accessibility activations opened/closed the mascot chat and routine controls without keyboard fallback.')
    }
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
    if ($CheckRoutine) {
        function Set-RoutinePermission([bool]$Enabled) {
            Open-RoutineControls
            $checkbox = Wait-For {
                $candidate = Find-Control 'Enable automatic verified backup' ([Windows.Automation.ControlType]::CheckBox)
                if ($candidate -and $candidate.Current.IsEnabled) { $candidate }
            } 'loaded routine permission'
            $toggle = $checkbox.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
            $wanted = if ($Enabled) { [Windows.Automation.ToggleState]::On } else { [Windows.Automation.ToggleState]::Off }
            if ($toggle.Current.ToggleState -ne $wanted) { $toggle.Toggle() }
            Click-Control 'Save routine permissions'
            $saved = if ($Enabled) { 'Automatic backup enabled' } else { 'Automatic backup disabled' }
            Wait-For { (Read-DesktopText) -match $saved } 'saved routine permission' | Out-Null
            Close-RoutineControls
        }
        function Assert-Backup {
            if (!(Test-Path -LiteralPath $source) -or !(Test-Path -LiteralPath $destination)) { throw 'Backup did not preserve both copies.' }
            foreach ($copy in @($source, $destination)) {
                if ((Get-FileHash -LiteralPath $copy -Algorithm SHA256).Hash -ne $expectedHash) { throw 'Backup bytes differ.' }
            }
            if (Find-Control 'Copy' ([Windows.Automation.ControlType]::Button)) { throw 'Routine backup unexpectedly required a per-file approval.' }
            if ((Read-DesktopText) -match 'never-observe-canary') { throw 'Routine backup exposed a private canary.' }
        }
        $backupRequest = 'Back up the largest file in "' + $sourceFolder + '" to "' + $DestinationRoot + '"'
        Send-Message $backupRequest
        Wait-For { (Read-DesktopText) -match 'Automatic backup is off' } 'default-off routine permission' | Out-Null
        Assert-Original
        $steps.Add('Routine backup defaulted off and changed no fixture files.')
        Set-RoutinePermission $true
        Send-Message $backupRequest
        Wait-For { (Read-DesktopText) -match 'Backup verified:' } 'automatic verified backup' 60 | Out-Null
        Assert-Backup
        $steps.Add('One native goal selected the permitted largest file and copied C:-to-D: without a per-file approval; both hashes matched.')
        Send-Message $backupRequest
        Wait-For { (Read-DesktopText) -match 'destination already exists' } 'backup conflict refusal' | Out-Null
        Assert-Backup
        $steps.Add('Repeating the backup refused to overwrite its existing destination.')
        Stop-Desktop
        Start-Desktop 'routine-restarted'
        Send-Message 'undo'
        Wait-For { (Read-DesktopText) -match 'Undid the verified file task' } 'backup undo after restart' | Out-Null
        Assert-Original
        Send-Message $backupRequest
        Wait-For { (Read-DesktopText) -match 'Backup verified:' } 'standing permission after restart' 60 | Out-Null
        Assert-Backup
        Send-Message 'undo'
        Wait-For { !(Test-Path -LiteralPath $destination) } 'second backup undo' | Out-Null
        Assert-Original
        $steps.Add('Standing permission and verified copy undo survived restart; undo removed only the copy.')
        if ($CheckAuthority) {
            function Set-RoutineScopes([string]$SourceScope, [string]$DestinationScope) {
                Open-RoutineControls
                foreach ($entry in @(
                    @{ Name = 'Allowed routine source folders'; Value = $SourceScope },
                    @{ Name = 'Allowed routine destination folders'; Value = $DestinationScope }
                )) {
                    $control = Wait-For {
                        $candidate = Find-Control $entry.Name ([Windows.Automation.ControlType]::Edit)
                        if ($candidate -and $candidate.Current.IsEnabled) { $candidate }
                    } "loaded scope field $($entry.Name)"
                    $control.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue($entry.Value)
                }
                Click-Control 'Save routine permissions'
                Wait-For { (Read-DesktopText) -match 'Automatic backup enabled' } 'saved routine resource scopes' | Out-Null
                Close-RoutineControls
            }
            Set-RoutineScopes $sourceFolder $DestinationRoot
            $outsideDestination = Join-Path $fixture.FullName 'outside-grant'
            New-Item -ItemType Directory -Path $outsideDestination | Out-Null
            Send-Message ('Back up the largest file in "' + $sourceFolder + '" to "' + $outsideDestination + '"')
            Wait-For { (Read-DesktopText) -match 'outside the folders allowed' } 'out-of-scope destination refusal' | Out-Null
            if ((Get-ChildItem -LiteralPath $outsideDestination).Count -ne 0) { throw 'Out-of-scope request created output.' }
            Assert-Original
            Send-Message $backupRequest
            Wait-For { (Read-DesktopText) -match 'Backup verified:' } 'in-scope automatic backup' 60 | Out-Null
            Assert-Backup
            Send-Message 'undo'
            Wait-For { !(Test-Path -LiteralPath $destination) } 'scoped copy undo' | Out-Null
            Stop-Desktop
            Start-Desktop 'scopes-restarted'
            Send-Message ('Back up the largest file in "' + $sourceFolder + '" to "' + $outsideDestination + '"')
            Wait-For { (Read-DesktopText) -match 'outside the folders allowed' } 'persisted scope refusal after restart' | Out-Null
            Assert-Original
            Set-RoutineScopes '' ''
            $steps.Add('Resource scopes blocked an outside destination before copying, allowed verified in-scope backup and undo, and remained enforced after restart. Empty scopes restored explicitly requested local access.')
        }
        Send-Message ('Back up the largest file in "all drives" to "' + $DestinationRoot + '"')
        Wait-For { (Read-DesktopText) -match 'Finding the largest accessible|Backup: checked' } 'whole-PC backup discovery' | Out-Null
        Send-Message 'How much free space?'
        Wait-For { (Read-DesktopText) -match "Here.s your storage" } 'independent answer during backup discovery' | Out-Null
        Click-Control 'Stop current work'
        Wait-For { (Read-DesktopText) -match 'Backup did not start: Backup stopped before copying' } 'backup discovery cancellation' 30 | Out-Null
        Assert-Original
        $steps.Add('Whole-PC backup discovery answered an independent question and stopped before copying.')
        Send-Message ('Back up the largest file in "all drives" to "' + $DestinationRoot + '"')
        Wait-For { (Read-DesktopText) -match 'Finding the largest accessible|Backup: checked' } 'backup discovery before revocation' | Out-Null
        Set-RoutinePermission $false
        Send-Message $backupRequest
        Wait-For { (Read-DesktopText) -match 'Automatic backup is off' } 'revoked routine permission' | Out-Null
        Assert-Original
        $steps.Add('Revoking the standing permission stopped pending discovery and prevented subsequent automatic backup.')
    }
    if ($CheckWorkflow) {
        function Set-CollectionPermission([bool]$Enabled) {
            Open-RoutineControls
            $checkbox = Wait-For {
                $candidate = Find-Control 'Enable automatic verified collections' ([Windows.Automation.ControlType]::CheckBox)
                if ($candidate -and $candidate.Current.IsEnabled) { $candidate }
            } 'loaded collection permission'
            $toggle = $checkbox.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
            $wanted = if ($Enabled) { [Windows.Automation.ToggleState]::On } else { [Windows.Automation.ToggleState]::Off }
            if ($toggle.Current.ToggleState -ne $wanted) { $toggle.Toggle() }
            Click-Control 'Save routine permissions'
            $saved = if ($Enabled) { 'Verified collections enabled' } else { 'Verified collections disabled' }
            Wait-For { (Read-DesktopText) -match $saved } 'saved collection permission' | Out-Null
            Close-RoutineControls
        }
        function Assert-Collection([string[]]$Names) {
            foreach ($name in $Names) {
                $original = Join-Path $sourceFolder $name
                $copy = Join-Path $DestinationRoot $name
                if (!(Test-Path -LiteralPath $original) -or !(Test-Path -LiteralPath $copy)) { throw "Missing collection copy: $name" }
                if ((Get-FileHash -LiteralPath $original).Hash -ne (Get-FileHash -LiteralPath $copy).Hash) { throw "Collection hash differs: $name" }
            }
            if ((Read-DesktopText) -match 'never-observe-canary') { throw 'Collection exposed a private canary.' }
            if (Find-Control 'Copy' ([Windows.Automation.ControlType]::Button)) { throw 'Collection required per-file approval.' }
            [IO.File]::WriteAllText((Join-Path $fixture.FullName ('workflow-evidence-' + [Guid]::NewGuid().ToString('N') + '.txt')), (Read-DesktopText))
        }
        $documents = 'Collect documents from "' + $sourceFolder + '" to "' + $DestinationRoot + '"'
        Send-Message $documents
        Wait-For { (Read-DesktopText) -match 'Automatic collections are off' } 'separate default-off collection grant' | Out-Null
        Assert-Original
        $steps.Add('Collections defaulted off separately from the single-file backup grant.')
        Set-CollectionPermission $true
        Send-Message $documents
        Wait-For { (Read-DesktopText) -match 'Workflow verified: 2 copies' } 'actual Qwen document workflow' 180 | Out-Null
        Assert-Collection @('acceptance-largest.txt', 'small.txt')
        if ((Test-Path -LiteralPath (Join-Path $DestinationRoot 'acceptance-photo.jpg')) -or (Test-Path -LiteralPath (Join-Path $DestinationRoot 'never-observe-canary.txt'))) { throw 'Document selection copied an excluded file.' }
        $steps.Add('Real Qwen proposed a document collection; native execution verified two copies and preserved originals/private exclusions.')
        if ($CheckCoordinator) {
            Wait-For { (Read-DesktopText) -match '3 of 3 steps recorded' } 'completed coordinator evidence in task view' 30 | Out-Null
            Click-Control 'Execution evidence · 3 of 3 steps recorded'
            Wait-For { (Read-DesktopText) -match 'Independently recheck completed copies' } 'expanded native completion evidence' | Out-Null
            [IO.File]::WriteAllText((Join-Path $fixture.FullName 'coordinator-task-view.txt'), (Read-DesktopText))
            Click-Control 'Inspect latest collection'
            Wait-For { (Read-DesktopText) -match 'Workflow .*: Completed' } 'task-view recovery inspection' 60 | Out-Null
            $steps.Add('The task view retained all three native steps, receipt evidence and completion checks; its recovery control inspected the latest real collection.')
        }
        if ($CheckLanguage) {
            Send-Message 'undo workflow'
            Wait-For { !(Test-Path -LiteralPath $destination) -and !(Test-Path -LiteralPath (Join-Path $DestinationRoot 'small.txt')) } 'clear verified collection for language case' 60 | Out-Null
            Send-Message ('In "' + $DestinationRoot + '" put copies of the documents from "' + $sourceFolder + '"')
            Wait-For { (Test-Path -LiteralPath $destination) -and (Test-Path -LiteralPath (Join-Path $DestinationRoot 'small.txt')) } 'model interpreted reversed location order' 180 | Out-Null
            Wait-For { !(Find-Control 'Stop current work' ([Windows.Automation.ControlType]::Button)) } 'completed reversed-location goal' 60 | Out-Null
            Assert-Collection @('acceptance-largest.txt', 'small.txt')
            $steps.Add('Actual Qwen interpreted reversed location order; native identity binding copied the correct documents and preserved source data.')
        }
        Send-Message $documents
        Wait-For { (Read-DesktopText) -match 'Workflow not completed: the destination already exists' } 'batch conflict refusal' 180 | Out-Null
        Assert-Collection @('acceptance-largest.txt', 'small.txt')
        $steps.Add('A repeated planned collection refused existing destinations without replacing either output.')
        Stop-Desktop
        Start-Desktop 'workflow-restarted'
        Open-RoutineControls
        Click-Control 'Show workflow status'
        Wait-For { (Read-DesktopText) -match 'Workflow .*: Completed' } 'restart workflow receipt inspection' 60 | Out-Null
        Assert-Collection @('acceptance-largest.txt', 'small.txt')
        Open-RoutineControls
        Click-Control 'Undo workflow'
        Wait-For { (Read-DesktopText) -match 'Workflow undo verified' } 'group undo after restart' 60 | Out-Null
        Assert-Original
        if (Test-Path -LiteralPath (Join-Path $DestinationRoot 'small.txt')) { throw 'Group undo left a verified copy.' }
        $steps.Add('Restart inspected both step receipts without replay; one group undo removed only the verified copies.')
        Send-Message $documents
        Wait-For { (Read-DesktopText) -match 'Workflow verified: 2 copies' } 'new collection for partial undo check' 180 | Out-Null
        Assert-Collection @('acceptance-largest.txt', 'small.txt')
        $editedSmall = Join-Path $DestinationRoot 'small.txt'
        [IO.File]::WriteAllText($editedSmall, 'fixture user edit in a two-file workflow')
        Send-Message 'undo workflow'
        Wait-For { (Read-DesktopText) -match '1 verified copies removed. Changed, partial or unverified outputs remain' } 'selective group undo around an edited output' 60 | Out-Null
        if ([IO.File]::ReadAllText($editedSmall) -ne 'fixture user edit in a two-file workflow') { throw 'Partial group undo changed a user edit.' }
        if ([IO.File]::ReadAllText((Join-Path $sourceFolder 'small.txt')) -ne 'small') { throw 'Partial group undo changed the original small file.' }
        Assert-Original
        Move-Item -LiteralPath $editedSmall -Destination (Join-Path $DestinationRoot 'retained-small-user-edit.txt')
        $steps.Add('With one edited output, group undo removed the other verified copy and preserved the user edit and both originals.')
        Send-Message ('Please gather all my photos in "' + $sourceFolder + '" into "' + $DestinationRoot + '"')
        Wait-For { (Read-DesktopText) -match 'Workflow verified: 1 copies' } 'real Qwen image workflow' 180 | Out-Null
        Assert-Collection @('acceptance-photo.jpg')
        Assert-Original
        Send-Message 'undo workflow'
        Wait-For { !(Test-Path -LiteralPath (Join-Path $DestinationRoot 'acceptance-photo.jpg')) } 'photo workflow undo' 60 | Out-Null
        $steps.Add('The same engine executed an unfamiliar photo-collection request and undid its verified output.')
        Send-Message ('Could you copy the largest document from "' + $sourceFolder + '" to "' + $DestinationRoot + '"')
        Wait-For { Test-Path -LiteralPath $destination } 'largest-document workflow output' 180 | Out-Null
        Wait-For { (Read-DesktopText) -match 'filter_documents.*select_largest' } 'composed filter and selection plan' 60 | Out-Null
        Wait-For { !(Find-Control 'Stop current work' ([Windows.Automation.ControlType]::Button)) } 'completed largest-document workflow' 60 | Out-Null
        Assert-Collection @('acceptance-largest.txt')
        if (Test-Path -LiteralPath (Join-Path $DestinationRoot 'small.txt')) { throw 'Largest-document selection copied the smaller document.' }
        [IO.File]::WriteAllText($destination, 'fixture user edit after workflow')
        Send-Message 'show workflow status'
        Wait-For { (Read-DesktopText) -match 'Workflow .*: RecoveryRequired' } 'changed output recovery status' 60 | Out-Null
        Send-Message 'undo workflow'
        Wait-For { (Read-DesktopText) -match 'Changed, partial or unverified outputs remain' } 'changed output retained on undo' 60 | Out-Null
        if ([IO.File]::ReadAllText($destination) -ne 'fixture user edit after workflow') { throw 'Group undo removed or changed a user edit.' }
        # Preserve the edited synthetic output under a distinct fixture name so
        # subsequent legacy checks can assert the original destination is empty.
        Move-Item -LiteralPath $destination -Destination (Join-Path $DestinationRoot 'retained-user-edit.txt')
        Assert-Original
        $steps.Add('A third composition selected the largest document; changed-output inspection and undo retained the user edit.')
        Send-Message ('Collect documents except invoices from "' + $sourceFolder + '" to "' + $DestinationRoot + '"')
        Wait-For { (Read-DesktopText) -match 'Specify one file collection' } 'unsupported content filter clarification' | Out-Null
        Assert-Original
        $steps.Add('Unsupported selection semantics requested clarification without copying a substitute collection.')
        Send-Message ('Collect documents from "all drives" to "' + $DestinationRoot + '"')
        Wait-For { (Read-DesktopText) -match 'Discovering permitted files|Workflow discovery:' } 'whole-PC planned collection discovery' 180 | Out-Null
        Set-CollectionPermission $false
        Send-Message $documents
        Wait-For { (Read-DesktopText) -match 'Automatic collections are off' } 'revoked collection permission' | Out-Null
        Assert-Original
        $steps.Add('Whole-PC collection discovery respected permission revocation and could not execute a later request.')
    }
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
    $_ | Out-String | Set-Content -LiteralPath (Join-Path $fixture.FullName 'failure.txt')
    try { Read-DesktopText | Set-Content -LiteralPath (Join-Path $fixture.FullName 'failure-ui.txt') } catch { }
    throw
} finally {
    $steps | Set-Content -LiteralPath (Join-Path $fixture.FullName 'completed-checks.txt')
    if ($script:desktop -and !$script:desktop.HasExited) {
        Stop-Desktop
    }
    $env:CUSTODIAN_VERIFICATION_ROOT = $priorRoot
    $env:CUSTODIAN_LLAMA_GPU = $priorGpu
}
