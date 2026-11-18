'use strict';
// PreToolUse guard for the project's first invariant: read-only toward the
// game. Blocks file edits inside any "Hearts of Iron IV" folder (install,
// mods, user documents) and shell commands that write there, plus hand edits
// of src/hoi4companion/_native (only the build scripts produce it).
// Reading game files (grep, cat, Read) stays allowed. Exit 2 blocks the call.
let input='';
process.stdin.on('data',c=>input+=c).on('end',()=>{
 let call;try{call=JSON.parse(input);}catch{process.exit(0);}
 const tool=call.tool_name||'',args=call.tool_input||{};
 const norm=s=>String(s||'').replace(/\\/g,'/');
 const game=/hearts of iron iv/i;
 const native=/src\/hoi4companion\/_native\//i;
 const block=reason=>{process.stderr.write(`Blocked by project guard: ${reason}\n`);process.exit(2);};
 if(['Edit','Write','NotebookEdit','MultiEdit'].includes(tool)){
  const file=norm(args.file_path||args.notebook_path);
  if(game.test(file))block(`the game is read-only; refusing to modify ${file}`);
  if(native.test(file))block('src/hoi4companion/_native is build output; run the build scripts (deliver-build) instead');
 }
 if(tool==='Bash'||tool==='PowerShell'){
  const command=norm(args.command);
  if(!game.test(command))process.exit(0);
  // A write-type operation in a command that names a game folder.
  const writes=/(^|[\s;&|(])(rm|rmdir|mv|cp|touch|tee|truncate|ln|chmod|del|erase|ren|move|copy|xcopy|robocopy|mkdir|Remove-Item|Move-Item|Copy-Item|Rename-Item|New-Item|Set-Content|Add-Content|Out-File|Clear-Content)\b|\bsed\s+(-[a-zA-Z]*i|--in-place)|[^>]>{1,2}\s*["']?[^\s"'&|]*hearts of iron iv/i;
  if(writes.test(command))block('the game is read-only; this command looks like it writes inside a Hearts of Iron IV folder');
 }
 process.exit(0);
});
