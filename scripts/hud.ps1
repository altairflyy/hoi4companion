param([switch]$Preview,[int]$Port=8766)
$ErrorActionPreference='Stop'
$hudRoot=Split-Path -Parent $PSScriptRoot
$hudLocal=Join-Path $hudRoot '.local'
New-Item -ItemType Directory -Path $hudLocal -Force | Out-Null
$hudRecord=Join-Path $hudLocal 'hud-host.json'
try {
    if(Test-Path -LiteralPath $hudRecord){
        $existingHud=Get-Content -Raw -LiteralPath $hudRecord | ConvertFrom-Json
        $existingHost=Get-CimInstance Win32_Process -Filter ('ProcessId='+$existingHud.host_pid) -ErrorAction SilentlyContinue
        if($existingHost -and $existingHost.CommandLine -like '*scripts\hud.ps1*'){exit}
    }
    $hudBase='http://127.0.0.1:'+$Port
    $hudReady=$false
    try{$null=Invoke-RestMethod ($hudBase+'/api/guide') -TimeoutSec 2;$hudReady=$true}catch{}
    if(-not $hudReady){
        $env:PYTHONPATH=Join-Path $hudRoot 'src'
        $hudPython=(Get-Command py.exe -ErrorAction Stop).Source
        Start-Process -FilePath $hudPython -ArgumentList @('-3','-m','hoi4companion','--no-browser','--port',"$Port") -WorkingDirectory $hudRoot -WindowStyle Hidden -RedirectStandardOutput (Join-Path $hudLocal 'server.hud.out.log') -RedirectStandardError (Join-Path $hudLocal 'server.hud.err.log') | Out-Null
        $hudDeadline=(Get-Date).AddSeconds(45)
        while((Get-Date) -lt $hudDeadline){try{$null=Invoke-RestMethod ($hudBase+'/api/guide') -TimeoutSec 2;$hudReady=$true;break}catch{Start-Sleep -Milliseconds 500}}
        if(-not $hudReady){throw 'Il companion non risponde sulla porta locale.'}
    }
    $hudEdge=@((Join-Path ${env:ProgramFiles(x86)} 'Microsoft\Edge\Application\msedge.exe'),(Join-Path $env:ProgramFiles 'Microsoft\Edge\Application\msedge.exe')) | Where-Object {Test-Path -LiteralPath $_} | Select-Object -First 1
    if(-not $hudEdge){throw 'Microsoft Edge non trovato; il pannello resta disponibile a /hud nel browser.'}
    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class HoiHudWindow {
 [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left,Top,Right,Bottom; }
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int cx,int cy,uint flags);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint pid);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out RECT r);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int command);
 [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h,int index);
 private delegate bool EnumCallback(IntPtr h,IntPtr arg);
 [DllImport("user32.dll")] private static extern bool EnumWindows(EnumCallback cb,IntPtr arg);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] private static extern int GetWindowText(IntPtr h,StringBuilder text,int max);
 public static IntPtr Find(string prefix) {
  IntPtr found=IntPtr.Zero;int count=0;
  EnumWindows((h,arg)=>{var text=new StringBuilder(1024);GetWindowText(h,text,text.Capacity);if(text.ToString().StartsWith(prefix,StringComparison.Ordinal)){found=h;count++;}return true;},IntPtr.Zero);
  return count==1?found:IntPtr.Zero;
 }
}
'@
    $hudToken=[guid]::NewGuid().ToString('N')
    $hudProfile=Join-Path $hudLocal 'hud-browser'
    $hudArea=[System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
    $hudWidth=420;$hudHeight=570
    $hudX=[Math]::Max(0,$hudArea.Right-$hudWidth-20);$hudY=$hudArea.Top+90
    $hudUrl=$hudBase+'/hud?instance='+$hudToken
    $hudArgs=@('--app='+$hudUrl,('--user-data-dir="'+$hudProfile+'"'),'--no-first-run','--no-default-browser-check','--disable-extensions',('--window-size='+$hudWidth+','+$hudHeight),('--window-position='+$hudX+','+$hudY))
    Start-Process -FilePath $hudEdge -ArgumentList $hudArgs -WindowStyle Hidden | Out-Null
    $hudWindow=$null;$hudDeadline=(Get-Date).AddSeconds(25)
    while((Get-Date) -lt $hudDeadline){
        $hudFound=[HoiHudWindow]::Find('HOI4 Companion HUD '+$hudToken)
        if($hudFound -ne [IntPtr]::Zero){$hudWindow=$hudFound;break}
        Start-Sleep -Milliseconds 250
    }
    if(-not $hudWindow){throw 'Finestra HUD non identificata in modo univoco.'}
    $hudShown=$true
    [HoiHudWindow]::ShowWindow($hudWindow,4) | Out-Null
    [HoiHudWindow]::SetWindowPos($hudWindow,[IntPtr](-1),$hudX,$hudY,$hudWidth,$hudHeight,0x0010) | Out-Null
    @{host_pid=$PID;window_handle=$hudWindow.ToInt64();token=$hudToken;preview=[bool]$Preview;url=$hudUrl;topmost=([HoiHudWindow]::GetWindowLong($hudWindow,-20) -band 8) -eq 8;game_memory_access=$false} | ConvertTo-Json | Set-Content -LiteralPath $hudRecord -Encoding UTF8
    while([HoiHudWindow]::IsWindow($hudWindow)){
        $hudForeground=[HoiHudWindow]::GetForegroundWindow();[uint32]$hudForegroundPid=0
        [HoiHudWindow]::GetWindowThreadProcessId($hudForeground,[ref]$hudForegroundPid) | Out-Null
        $hudForegroundProcess=Get-Process -Id $hudForegroundPid -ErrorAction SilentlyContinue
        $hudInGame=$hudForegroundProcess -and $hudForegroundProcess.ProcessName -eq 'hoi4'
        $hudVisible=$Preview -or $hudInGame -or $hudForeground -eq $hudWindow
        if($hudVisible){
            if($hudInGame){$hudGameRect=New-Object HoiHudWindow+RECT;if([HoiHudWindow]::GetWindowRect($hudForeground,[ref]$hudGameRect)){$hudX=[Math]::Max($hudGameRect.Left,$hudGameRect.Right-$hudWidth-18);$hudY=$hudGameRect.Top+90}}
            if(-not $hudShown){[HoiHudWindow]::ShowWindow($hudWindow,4) | Out-Null;$hudShown=$true}
            [HoiHudWindow]::SetWindowPos($hudWindow,[IntPtr](-1),$hudX,$hudY,$hudWidth,$hudHeight,0x0010) | Out-Null
        }elseif($hudShown){[HoiHudWindow]::ShowWindow($hudWindow,0) | Out-Null;$hudShown=$false}
        Start-Sleep -Milliseconds 500
    }
}catch{
    $_ | Out-String | Set-Content -LiteralPath (Join-Path $hudLocal 'hud-error.log') -Encoding UTF8
}finally{
    if(Test-Path -LiteralPath $hudRecord){
        $record=Get-Content -Raw -LiteralPath $hudRecord | ConvertFrom-Json
        if($record.host_pid -eq $PID){Remove-Item -LiteralPath $hudRecord}
    }
}
