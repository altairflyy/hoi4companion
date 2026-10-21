param([ValidateRange(10,3600)][int]$Seconds=90,[switch]$TestOnly,[int]$AutoCancelAfterSeconds=0)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing -TypeDefinition @'
using System;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Runtime.InteropServices;
using System.Windows.Forms;
public static class HoiShutdownCountdown {
 [DllImport("user32.dll")] private static extern bool ShowWindow(IntPtr h,int command);
 public static void Run(int seconds,bool testOnly,int autoCancel,string record) {
  bool requested=false;DateTime start=DateTime.UtcNow,deadline=start.AddSeconds(seconds);
  var form=new Form {Text=testOnly?"TEST - Annullamento spegnimento":"Spegnimento PC - HOI4 Companion",Width=500,Height=295,StartPosition=FormStartPosition.CenterScreen,TopMost=true,FormBorderStyle=FormBorderStyle.FixedDialog,MaximizeBox=false,MinimizeBox=false};
  var headline=new Label {Left=22,Top=22,Width=445,Height=64,Font=new Font("Segoe UI",17),Text="Spegnimento del PC tra "+seconds+" secondi"};
  var help=new Label {Left=22,Top=94,Width=445,Height=78,Font=new Font("Segoe UI",10),Text=testOnly?"Prova dell'interfaccia: lo spegnimento e disabilitato anche alla scadenza.":"Puoi annullare con il pulsante o chiudendo questa finestra. Lo spegnimento non forza la chiusura dei programmi con lavoro non salvato."};
  var cancel=new Button {Text="Annulla spegnimento",Left=22,Top=182,Width=445,Height=43,Font=new Font("Segoe UI",11)};
  form.Controls.Add(headline);form.Controls.Add(help);form.Controls.Add(cancel);
  Action<string,int> log=(status,left)=>File.WriteAllText(record,"{\"status\":\""+status+"\",\"remaining_seconds\":"+left+",\"pid\":"+Process.GetCurrentProcess().Id+",\"forced\":false,\"test_only\":"+(testOnly?"true":"false")+"}");
  log("countdown",seconds);
  var timer=new Timer {Interval=1000};
  cancel.Click+=(s,e)=>form.Close();
  form.Shown+=(s,e)=>form.BeginInvoke(new Action(()=>{ShowWindow(form.Handle,5);form.Activate();}));
  timer.Tick+=(s,e)=>{
   int left=Math.Max(0,(int)Math.Ceiling((deadline-DateTime.UtcNow).TotalSeconds));
   headline.Text="Spegnimento del PC tra "+left+" secondi";log("countdown",left);
   if(testOnly&&autoCancel>0&&(DateTime.UtcNow-start).TotalSeconds>=autoCancel){cancel.PerformClick();return;}
   if(left==0){timer.Stop();requested=true;log(testOnly?"test_expired":"shutdown_requested",0);if(!testOnly)Process.Start(new ProcessStartInfo(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.System),"shutdown.exe"),"/s /t 0"){CreateNoWindow=true,UseShellExecute=false});form.Close();}
  };
  form.FormClosed+=(s,e)=>{timer.Stop();if(!requested)log("cancelled",Math.Max(0,(int)Math.Ceiling((deadline-DateTime.UtcNow).TotalSeconds)));};
  timer.Start();form.ShowDialog();timer.Dispose();form.Dispose();
 }
}
'@
$shutdownRoot=Split-Path -Parent $PSScriptRoot
$shutdownRecord=Join-Path $shutdownRoot '.local\shutdown-request.json'
[HoiShutdownCountdown]::Run($Seconds,[bool]$TestOnly,$AutoCancelAfterSeconds,$shutdownRecord)
