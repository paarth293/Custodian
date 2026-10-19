param([Parameter(Mandatory=$true)][string]$FixtureRoot,
    [string]$Editor='D:\paagu\Microsoft VS Code\Code.exe',
    [string]$FixtureExecutable='D:\Custodian\target\debug\examples\editor_adversarial_acceptance.exe',
    [switch]$Modal)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
if (![IO.Path]::IsPathFullyQualified($FixtureRoot) -or (Test-Path -LiteralPath $FixtureRoot)) { throw 'Fresh absolute fixture root required.' }
New-Item -ItemType Directory -Path $FixtureRoot | Out-Null
[IO.File]::WriteAllText((Join-Path $FixtureRoot '.custodian-fixture-root'),'')
$extension=Join-Path $FixtureRoot 'fixture-extension'
New-Item -ItemType Directory -Path $extension | Out-Null
[IO.File]::WriteAllText((Join-Path $extension 'package.json'),'{"name":"custodian-unsaved-fixture","publisher":"custodian-test-only","version":"0.0.1","engines":{"vscode":"1.140.0"},"main":"extension.cjs","activationEvents":["*"]}')
# Test-user controls only. Never staged, installed or admitted as a production adapter.
[IO.File]::WriteAllText((Join-Path $extension 'extension.cjs'),@'
const fs=require('node:fs'),path=require('node:path'),vscode=require('vscode');
exports.activate=async function(context) {
 const root=process.env.CUSTODIAN_UNSAVED_FIXTURE;
 if(!root||!fs.existsSync(path.join(root,'.custodian-fixture-root')))return;
 const file=path.join(root,'document.txt');
 const doc=await vscode.workspace.openTextDocument(file);
 await vscode.window.showTextDocument(doc);
 const edit=new vscode.WorkspaceEdit();
 edit.insert(doc.uri,new vscode.Position(0,0),'Unsaved unrelated note: preserve exactly.');
 if(!await vscode.workspace.applyEdit(edit))throw Error('Fixture edit failed');
 const expected=doc.getText();
 if(!doc.isDirty)throw Error('Fixture must be unsaved');
 fs.writeFileSync(path.join(root,'ready'),JSON.stringify({pid:process.pid,parent:process.ppid}));
 const timer=setInterval(async()=>{
  if(!fs.existsSync(path.join(root,'finish')))return;
  clearInterval(timer);
  if(doc.getText()!==expected||!doc.isDirty)throw Error('Unrelated unsaved work changed');
  fs.writeFileSync(path.join(root,'preserved'),'');
  if(!await doc.save())throw Error('Fixture preservation save failed');
  const tabs=vscode.window.tabGroups.all.flatMap(g=>g.tabs);
  if(tabs.length!==1||tabs[0].input.uri?.toString()!==doc.uri.toString())throw Error('Unexpected fixture tab');
  if(!await vscode.window.tabGroups.close(tabs[0]))throw Error('Fixture tab close failed');
  fs.writeFileSync(path.join(root,'closed'),'');
  await vscode.commands.executeCommand('workbench.action.closeWindow');
 },100);
 context.subscriptions.push({dispose(){clearInterval(timer);}});
};
'@)
[IO.File]::WriteAllText((Join-Path $FixtureRoot 'document.txt'),'')
if (!('CustodianInterferenceWindows' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class CustodianInterferenceWindows {
 [StructLayout(LayoutKind.Sequential)] struct Rect { public int Left,Top,Right,Bottom; }
 delegate bool Callback(IntPtr hwnd,IntPtr arg);
 [DllImport("user32.dll")] static extern bool EnumWindows(Callback cb,IntPtr arg);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,System.Text.StringBuilder text,int size);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd,int command);
 [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr hwnd,out Rect rect);
 [DllImport("user32.dll")] static extern IntPtr GetWindow(IntPtr hwnd,uint command);
 [DllImport("user32.dll")] static extern bool PostMessage(IntPtr hwnd,uint message,IntPtr w,IntPtr l);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int MessageBox(IntPtr owner,string text,string title,uint flags);
 [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr hwnd);
 public static System.Threading.Tasks.Task<int> Modal(IntPtr owner) {
  return System.Threading.Tasks.Task.Run(()=>MessageBox(owner,"Synthetic unknown prompt: the operator must yield.","Custodian test-only modal",1));
 }
 public static void CloseOwnModal(IntPtr owner) {
  IntPtr result=IntPtr.Zero;uint self=(uint)System.Diagnostics.Process.GetCurrentProcess().Id;
  EnumWindows((hwnd,arg)=>{uint pid;GetWindowThreadProcessId(hwnd,out pid);if(pid==self&&GetWindow(hwnd,4)==owner){result=hwnd;return false;}return true;},IntPtr.Zero);
  if(result==IntPtr.Zero||!PostMessage(result,0x10,IntPtr.Zero,IntPtr.Zero))throw new Exception("Synthetic modal cleanup unproved");
 }
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd,IntPtr after,int x,int y,int cx,int cy,uint flags);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 public static IntPtr OwnWindow(uint expected) {
  IntPtr result=IntPtr.Zero;
  EnumWindows((hwnd,arg)=>{uint pid;GetWindowThreadProcessId(hwnd,out pid);var name=new System.Text.StringBuilder(64);GetClassName(hwnd,name,64);Rect rect;if(pid==expected&&name.ToString()=="Chrome_WidgetWin_1"&&GetWindowRect(hwnd,out rect)&&rect.Right-rect.Left>300&&rect.Bottom-rect.Top>200){result=hwnd;return false;}return true;},IntPtr.Zero);
  return result;
 }
}
'@
}
function Wait-Fixture([scriptblock]$Condition,[int]$Seconds=40) {
 $limit=[DateTime]::UtcNow.AddSeconds($Seconds)
 while (!( &$Condition )) { if([DateTime]::UtcNow -gt $limit){throw 'Fixture condition timed out; preserve test windows for review.'}; Start-Sleep -Milliseconds 100 }
}
$prior=$env:CUSTODIAN_UNSAVED_FIXTURE
$priorRoot=$env:CUSTODIAN_EDITOR_FAULT_ROOT
$env:CUSTODIAN_UNSAVED_FIXTURE=$FixtureRoot
$env:CUSTODIAN_EDITOR_FAULT_ROOT=Join-Path $FixtureRoot 'target-task'
$canary=$null
try {
 $arguments=@('--new-window','--user-data-dir',('"'+(Join-Path $FixtureRoot 'profile')+'"'),'--extensions-dir',('"'+(Join-Path $FixtureRoot 'extensions')+'"'),('--extensionDevelopmentPath="'+$extension+'"'),'--disable-updates','--disable-telemetry',('"'+(Join-Path $FixtureRoot 'document.txt')+'"'))
 $canary=Start-Process -FilePath $Editor -ArgumentList $arguments -WindowStyle Hidden -PassThru
 Wait-Fixture { Test-Path -LiteralPath (Join-Path $FixtureRoot 'ready') }
 $hello=Get-Content -LiteralPath (Join-Path $FixtureRoot 'ready') -Raw | ConvertFrom-Json
 $actual=Get-Process -Id $hello.parent
 if($actual.StartTime -lt $canary.StartTime.AddSeconds(-1)){throw 'Fixture parent process predates this test.'}
 $canary=$actual
 $mode=if($Modal){'native-modal'}else{'duplicate-focus'}
 $fixture=Start-Process -FilePath $FixtureExecutable -ArgumentList $mode -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $FixtureRoot 'target-result.txt')
 $identityPath=Join-Path $env:CUSTODIAN_EDITOR_FAULT_ROOT 'fixture-identity.json'
 Wait-Fixture { Test-Path -LiteralPath $identityPath }
 $identity=Get-Content -LiteralPath $identityPath -Raw | ConvertFrom-Json
 $other=[CustodianInterferenceWindows]::OwnWindow($canary.Id)
 if($other -eq [IntPtr]::Zero){throw 'No exact fixture window.'}
 [void][CustodianInterferenceWindows]::ShowWindow($other,9)
 if(![CustodianInterferenceWindows]::SetWindowPos([IntPtr]$identity.window,[IntPtr]::Zero,70,90,740,520,0x14)){throw 'Fixture move failed.'}
 if($Modal) {
  $modalTask=[CustodianInterferenceWindows]::Modal([IntPtr]$identity.window)
  Wait-Fixture { ![CustodianInterferenceWindows]::IsWindowEnabled([IntPtr]$identity.window) } 10
 } else {
  [void][CustodianInterferenceWindows]::SetForegroundWindow($other)
  $element=[System.Windows.Automation.AutomationElement]::FromHandle($other)
  $focusable=$element.FindFirst([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::IsKeyboardFocusableProperty,$true))
  if($null -eq $focusable){throw 'Fixture has no focusable control.'}
  $focusable.SetFocus()
  Wait-Fixture { [CustodianInterferenceWindows]::GetForegroundWindow() -eq $other } 5
 }
 [IO.File]::WriteAllText((Join-Path $env:CUSTODIAN_EDITOR_FAULT_ROOT 'interference-ready'),'')
 if($Modal) {
  Wait-Fixture { Test-Path -LiteralPath (Join-Path $env:CUSTODIAN_EDITOR_FAULT_ROOT 'modal-refused') } 20
  [CustodianInterferenceWindows]::CloseOwnModal([IntPtr]$identity.window)
  if(!$modalTask.Wait(5000)){throw 'Synthetic modal exit unproved.'}
  [IO.File]::WriteAllText((Join-Path $env:CUSTODIAN_EDITOR_FAULT_ROOT 'modal-dismissed'),'')
 }
 if(!$fixture.WaitForExit(60000) -or $fixture.ExitCode -ne 0){throw 'Bound document workflow failed.'}
 [IO.File]::WriteAllText((Join-Path $FixtureRoot 'finish'),'')
 Wait-Fixture { Test-Path -LiteralPath (Join-Path $FixtureRoot 'closed') }
 if(!$canary.WaitForExit(20000)){throw 'Fixture editor normal exit not proved.'}
 if(!(Test-Path -LiteralPath (Join-Path $FixtureRoot 'preserved'))){throw 'Unrelated note preservation not proved.'}
 Write-Output "PASS: $mode; duplicate names, moved target, unrelated unsaved text preserved and normal cleanup."
} finally {
 [IO.File]::WriteAllText((Join-Path $FixtureRoot 'finish'),'')
 $env:CUSTODIAN_UNSAVED_FIXTURE=$prior
 $env:CUSTODIAN_EDITOR_FAULT_ROOT=$priorRoot
 # No forced termination of editor processes, guessed dialogs or global keys.
}
