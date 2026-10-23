Option Explicit
Dim shell,fso,root,command,port
Set shell=CreateObject("WScript.Shell")
Set fso=CreateObject("Scripting.FileSystemObject")
root=fso.GetParentFolderName(WScript.ScriptFullName)
shell.CurrentDirectory=root
shell.Environment("PROCESS")("PYTHONPATH")=fso.BuildPath(root,"src")
port=8772
If WScript.Arguments.Count>0 Then
    If Not IsNumeric(WScript.Arguments(0)) Then WScript.Quit 1
    port=CLng(WScript.Arguments(0))
    If port<1024 Or port>65535 Then WScript.Quit 1
End If
command="pyw.exe -3 -m hoi4companion.overlay_windows --port " & port & " --start"
shell.Run command,0,False
