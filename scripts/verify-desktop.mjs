import { spawn, spawnSync, execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, copyFileSync, existsSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { createServer } from 'node:net';
import { desktopOptions, containedPath, artifactDirectory, windowsTools } from './desktop-harness-paths.mjs';
import { createCdpEvaluator, dispatchCdpMessage } from './cdp-messages.mjs';
if (process.platform !== 'win32') throw new Error('This WebView2 harness runs on Windows. Other desktop webviews need their native validation lane.');
const repository = resolve(import.meta.dirname, '..');
const options = desktopOptions(process.argv.slice(2), repository);
const executable = containedPath(repository, options.executable);
const { tag } = options;
const tools = windowsTools();
const artifact = artifactDirectory(repository);
const run = mkdtempSync(join(artifact,`desktop-${tag}-`));
const fixture = join(run,'history with spaces'); mkdirSync(fixture);
const env = {...process.env, ComSpec:tools.command, PATH:[join(tools.git,'..'),join(tools.command,'..')].join(';'), GIT_CONFIG_GLOBAL:'NUL', GIT_CONFIG_SYSTEM:'NUL', GIT_TERMINAL_PROMPT:'0'};
function git(args, cwd=fixture, input) { const result=spawnSync(tools.git,args,{cwd,env,input,encoding:'utf8',windowsHide:true,maxBuffer:32*1024*1024,timeout:60_000}); if(result.status!==0)throw new Error(`git ${args[0]} failed: ${result.stderr}`);return result.stdout.trim(); }
git(['init','-q','-b','main']);git(['config','user.name','Desktop Fixture']);git(['config','user.email','desktop@example.invalid']);git(['config','commit.gpgsign','false']);git(['config','core.autocrlf','false']);
const content=Array.from({length:1000},(_,i)=>`const line${i} = ${i};`).join('\n')+'\n';
let input=`blob\nmark :1\ndata ${Buffer.byteLength(content)}\n${content}\n`;
for(let i=0;i<50000;i++){const message=`History ${i}\n`;input+=`commit refs/heads/main\nmark :${i+2}\nauthor Desktop Fixture <desktop@example.invalid> ${1700000000+i} +0000\ncommitter Desktop Fixture <desktop@example.invalid> ${1700000000+i} +0000\ndata ${Buffer.byteLength(message)}\n${message}${i?`from :${i+1}\n`:''}M 100644 :1 tracked.txt\n\n`;}
// The raw fixture stream uses real line endings, independently of shell escaping.
input=input.replaceAll('\\n','\n');
git(['fast-import','--quiet'],fixture,input);git(['reset','--hard','HEAD']);
copyFileSync(join(repository,'icons/32x32.png'),join(fixture,'preview.png'));
const watchPaths=[];
for(let n=0;n<2;n++){const path=join(run,`watch-${n}`);mkdirSync(path);git(['init','-q','-b','main'],path);for(let d=0;d<40;d++){const dir=join(path,`dir-${d}`);mkdirSync(dir);for(let f=0;f<50;f++)writeFileSync(join(dir,`file-${f}.txt`),'fixture\n');}watchPaths.push(path);}
const reserve=createServer();await new Promise(r=>reserve.listen(0,'127.0.0.1',r));const port=reserve.address().port;await new Promise(r=>reserve.close(r));
const profile=join(run,'webview-profile');mkdirSync(profile);
const child=spawn(executable,['--screenshot',join(run,'desktop.png'),'--shot-view','validation','--shot-size','1200x850'],{cwd:repository,windowsHide:true,stdio:['ignore','pipe','pipe'],env:{...env,LOCALAPPDATA:join(run,'userdata'),WEBVIEW2_USER_DATA_FOLDER:profile,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`}});
let log='';child.stdout.on('data',b=>log+=b);child.stderr.on('data',b=>log+=b);
const sleep=ms=>new Promise(r=>setTimeout(r,ms));let socket;const report={tag,fixtureCommits:50000,checks:[],exceptions:[],memory:[]};
let send;
const evaluate=createCdpEvaluator((method,params)=>send(method,params));
async function until(code,label,timeout=30_000){const started=Date.now();while(Date.now()-started<timeout){if(await evaluate(`return !!(${code});`))return;await sleep(50);}throw new Error(`Timed out: ${label}`);}
async function check(id,fn){const started=Date.now();try{const data=await fn();report.checks.push({id,status:'passed',milliseconds:Date.now()-started,...(data??{})});}catch(error){report.checks.push({id,status:'failed',error:String(error)});throw error;}}
function powershell(code,extra={}){return execFileSync(tools.powershell,['-NoProfile','-NonInteractive','-EncodedCommand',Buffer.from("[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new();$ProgressPreference='SilentlyContinue';"+code,'utf16le').toString('base64')],{encoding:'utf8',windowsHide:true,env:{...process.env,...extra},timeout:20_000}).trim();}
function memory(label){const text=powershell(`$all=Get-CimInstance Win32_Process; $ids=[System.Collections.Generic.HashSet[int]]::new(); [void]$ids.Add(${child.pid}); do { $before=$ids.Count; foreach($p in $all){if($ids.Contains([int]$p.ParentProcessId)){[void]$ids.Add([int]$p.ProcessId)}} } while($ids.Count -ne $before); $items=@(foreach($id in $ids){$p=Get-Process -Id $id -ErrorAction SilentlyContinue;if($p){[pscustomobject]@{name=$p.ProcessName; privateBytes=$p.PrivateMemorySize64; workingSetBytes=$p.WorkingSet64}}}); ConvertTo-Json -Compress -InputObject $items`);const processes=JSON.parse(text||'[]');const sample={label,processes,privateBytes:processes.reduce((s,p)=>s+p.privateBytes,0),workingSetBytes:processes.reduce((s,p)=>s+p.workingSetBytes,0)};report.memory.push(sample);return sample;}
try{
 let pages;
 for(let i=0;i<200;i++){try{pages=await(await fetch(`http://127.0.0.1:${port}/json/list`)).json();if(pages.some(p=>p.type==='page'))break;}catch{}if(child.exitCode!==null)throw new Error(`App exited: ${log}`);await sleep(100);}
 if(!pages?.some(p=>p.type==='page'))throw new Error('WebView2 debugging endpoint did not appear');
 socket=new WebSocket(pages.find(p=>p.type==='page').webSocketDebuggerUrl);await new Promise((r,j)=>{socket.onopen=r;socket.onerror=j;});
 let sequence=0;const pending=new Map();socket.onmessage=e=>dispatchCdpMessage(e.data,pending,report.exceptions);
 send=(method,params={})=>new Promise((r,j)=>{const id=++sequence;const timer=setTimeout(()=>{pending.delete(id);j(new Error(`CDP ${method} timed out`));},90_000);pending.set(id,value=>{clearTimeout(timer);r(value);});socket.send(JSON.stringify({id,method,params}));});
 await send('Runtime.enable');await send('Page.enable');
 await until('window.__gitcraftVerification','isolated validation startup');
 await evaluate(`window.v=window.__gitcraftVerification; window.fixture=input.fixture; window.violations=[]; document.addEventListener('securitypolicyviolation',e=>violations.push({directive:e.violatedDirective,blocked:e.blockedURI})); v.store.getState().setSettings({authorInitials:true,autoUpdate:false,autoFetchMin:0,lazyLoad:false,initialCommits:2000});`,{fixture});
 await evaluate(`await v.invoke('save_settings',{settings:{settings:{gitPath:input.gitPath,sshUseAgent:true}}});`,{gitPath:tools.git});
 report.accountAvailability=await evaluate(`return await v.invoke('desktop_validation_accounts');`);
 await check('O001 native 50000-commit loading',async()=>{
  const result=await evaluate(`const started=performance.now();await v.openRepoPath(fixture);window.path=v.store.getState().tabs.find(t=>t.id===v.store.getState().activeId).path;return {initialMs:performance.now()-started,commits:v.repoData(path).commits.length};`);
  await until('document.querySelectorAll(".graph-row").length > 0','native graph');
  await evaluate(`for(let i=0;i<4;i++)await v.loadMoreRepo(path);window.selected=v.repoData(path).commits[9000].sha;window.tabId=v.store.getState().activeId;v.store.getState().setUi(tabId,{selection:{type:'commit',sha:selected}});`);
  return {...result,loaded:await evaluate('return v.repoData(path).commits.length;')};
 });
 await check('O033 packaged first editor and language coverage',async()=>{
  memory('before-first-editor');
  await evaluate(`window.editorStarted=performance.now();v.store.getState().setSettings({diffMode:'split'});v.store.getState().setUi(tabId,{diff:{source:{kind:'Unstaged'},file:'tracked.txt',untracked:false,parent:null}});`);
  await until('document.querySelector(".monaco-diff-editor")','first editor');
  const result=await evaluate(`const readyMs=performance.now()-editorStarted;window.editorApi=await v.editor();const files={'a.ts':'typescript','a.js':'javascript','a.json':'json','a.rs':'rust','a.py':'python','a.css':'css','a.html':'html','a.md':'markdown','Dockerfile':'dockerfile','a.yaml':'yaml','a.sh':'shell'};const languages=Object.fromEntries(Object.keys(files).map(f=>[f,editorApi.languageFor(f)]));if(Object.keys(files).some(f=>files[f]!==languages[f]))throw new Error('Language coverage: '+JSON.stringify(languages));return {readyMs,languages,registeredLanguages:editorApi.monaco.languages.getLanguages().length};`);
  await until('editorApi.monaco.editor.getDiffEditors()[0]?.getLineChanges() !== null','diff computation');
  const editor=await evaluate(`const e=editorApi.monaco.editor.getDiffEditors()[0];e.getModifiedEditor().trigger('fixture','actions.find',null);return {hasFind:!!e.getModifiedEditor().getAction('actions.find'),hasClipboard:!!e.getModifiedEditor().getAction('editor.action.clipboardCopyWithSyntaxHighlightingAction')};`);
  if(!editor.hasFind||!editor.hasClipboard)throw new Error('Editor shortcut actions missing');memory('after-first-editor');return {...result,...editor};
 });
 await check('S004 packaged IPC workers and image sources',async()=>{
  return evaluate(`const preview=await v.api.fileVersions(path,{kind:'Unstaged'},'preview.png',true,null,false);if(!preview.new_image)throw new Error('Native image preview missing');await new Promise((resolve,reject)=>{const image=new Image();image.onload=resolve;image.onerror=reject;image.src=preview.new_image;});const json=editorApi.monaco.editor.createModel('{"a":1}','json');const host=document.createElement('div');host.style.cssText='width:400px;height:150px';document.body.append(host);const jsonEditor=editorApi.monaco.editor.create(host,{model:json});try{await new Promise(r=>setTimeout(r,300));const worker=await editorApi.monaco.languages.json.getWorker();const proxy=await worker(json.uri);await proxy.doValidation(json.uri.toString());}finally{jsonEditor.setModel(null);jsonEditor.dispose();json.dispose();host.remove();}v.store.getState().setDialog({kind:'prefs'});return {dataImage:true,jsonWorker:true,violations:[...violations]};`);
 });
 await evaluate(`v.store.getState().setDialog(null);v.store.getState().setUi(tabId,{diff:null});`);
 await check('S004 preferences avatars and native updater configuration',async()=>{
  await evaluate(`v.store.getState().setDialog({kind:'prefs'});`);await until('document.querySelector("section[aria-label=Preferences]")','preferences screen');
  const result=await evaluate(`window.avatarProbe=new Image();avatarProbe.src='https://www.gravatar.com/avatar/00000000000000000000000000000000?d=identicon';document.body.append(avatarProbe);const config=await v.invoke('update_config');await v.updates();return {updaterConfigured:config.configured};`);
  await sleep(300);await evaluate(`avatarProbe.remove();v.store.getState().setDialog(null);`);return {...result,avatarSourceAllowed:true};
 });
 await check('O001 action completion and history restoration',async()=>{
  writeFileSync(join(fixture,'tracked.txt'),content+'// desktop action\n');
  const result=await evaluate(`window.actionStarted=performance.now();const ok=await v.act(path,{type:'commit',summary:'Desktop validation commit',description:'',amend:false,stageAll:true,skipHooks:true,stripComments:false});if(!ok)throw new Error('Commit action failed');return {actionMs:performance.now()-actionStarted};`);
  await until('v.repoData(path)?.commits.some(c=>c.sha===selected)','selected old history restored',60_000);
  return {...result,...await evaluate(`return {restoredMs:performance.now()-actionStarted,commits:v.repoData(path).commits.length,selection:v.store.getState().ui[tabId].selection.sha===selected};`)};
 });
 await check('O034 concurrent native watcher startup close and late completion',async()=>{
  return evaluate(`const paths=input.watchPaths;window.changed=[];const stop=await v.listen('repo-changed',e=>changed.push(e.payload.path));const started=performance.now();const first=v.api.watchRepo(paths[0]);const second=v.api.watchRepo(paths[1]);await new Promise(r=>setTimeout(r,10));const closing=performance.now();await v.api.unwatchRepo(paths[0]);const closeMs=performance.now()-closing;await Promise.all([first,second]);await v.api.unwatchRepo(paths[1]);stop();return {closeMs,totalMs:performance.now()-started,state:await v.invoke('desktop_validation_state')};`,{watchPaths});
 });
 await check('O013 visible and hidden native terminals',async()=>{
  memory('before-terminals');
  await evaluate(`window.terminalOutput=new Map();window.termStop=await v.listen('term-data',e=>{const text=(terminalOutput.get(e.payload.id)||'')+e.payload.data;terminalOutput.set(e.payload.id,text.slice(-200000));});v.store.getState().setSettings({termShell:'cmd',showTerminal:true});`);
  await until('document.querySelector("[data-terminal-session]")','first native terminal');
  for(let i=0;i<4;i++){await evaluate(`document.querySelector('[title="New terminal tab"]').click();`);await until(`document.querySelectorAll('[data-terminal-session]').length===${i+2}`,'new terminal');}
  await sleep(500);memory('five-terminals-one-visible');
  const result=await evaluate(`const samples=[];const ids=[...document.querySelectorAll('[data-terminal-session]')].map(e=>Number(e.dataset.terminalSession));for(const id of ids){const marker='GCROUNDTRIP'+id;terminalOutput.set(id,'');const start=performance.now();await v.invoke('term_write',{id,data:'echo '+marker+'\\r'});while((terminalOutput.get(id)||'').split(marker).length<3){if(performance.now()-start>5000)throw new Error('Terminal roundtrip timeout');await new Promise(r=>setTimeout(r,5));}samples.push({id,roundTripMs:performance.now()-start});}v.store.getState().setSettings({showTerminal:false});return {samples,state:await v.invoke('desktop_validation_state')};`);
  await sleep(200);memory('five-terminals-hidden');
  result.hiddenOutput=await evaluate(`const id=[...document.querySelectorAll('[data-terminal-session]')].map(e=>Number(e.dataset.terminalSession))[0];terminalOutput.set(id,'');await v.invoke('term_write',{id,data:'for /L %i in (1,1,2000) do @echo GCSTRESS01234567890123456789012345678901234567890123456789\\r'});const start=performance.now();while((terminalOutput.get(id)||'').split('GCSTRESS').length<2001){if(performance.now()-start>10000)throw new Error('Hidden output stalled');await new Promise(r=>setTimeout(r,10));}const state=await v.invoke('desktop_validation_state');if(state.terminals.some(([,pending])=>pending>65536))throw new Error('Terminal output budget exceeded');return {characters:terminalOutput.get(id).length,state};`);

  await until('(await v.invoke("desktop_validation_state")).terminals.every(([,pending])=>pending===0)','hidden terminal acknowledgements');
  result.hiddenOutput.state=await evaluate(`return await v.invoke('desktop_validation_state');`);
  await evaluate(`for(const id of [...document.querySelectorAll('[data-terminal-session]')].map(e=>Number(e.dataset.terminalSession)))await v.invoke('term_close',{id});termStop();`);
  return result;
 });
 await check('B066 Explorer selects nested spaced and Unicode paths',async()=>{
  const folder=join(run,'nested folder','caf\u00e9 \u65e5\u672c');mkdirSync(folder,{recursive:true});const file=join(folder,'selected \u03bb file.txt');writeFileSync(file,'selection fixture');
  const windows=()=>JSON.parse(powershell(`$shell=New-Object -ComObject Shell.Application; $items=@(foreach($w in $shell.Windows()){try{[pscustomobject]@{hwnd=[long]$w.HWND;folder=[string]$w.Document.Folder.Self.Path;selected=@($w.Document.SelectedItems() | ForEach-Object {[string]$_.Path})}}catch{}}); ConvertTo-Json -Depth 4 -Compress -InputObject $items`)||'[]');
  const before=new Set(windows().map(w=>w.hwnd));
  try {
   await evaluate(`await v.api.revealFile(input.folder,input.file);`,{folder,file:'selected \u03bb file.txt'});
   let selected=[];for(let n=0;n<12;n++){selected=windows().filter(w=>w.selected.some(p=>p.toLowerCase()===file.toLowerCase()));if(selected.length)break;await sleep(150);}
   if(!selected.length)throw new Error('Explorer did not select the exact Unicode file');
   const unc='\\\\localhost\\'+file[0]+'$'+file.slice(2);const available=powershell(`Test-Path -LiteralPath '${unc.replaceAll("'", "''")}' -ErrorAction SilentlyContinue`)==='True';
   let uncSelected=false;if(available){await evaluate(`await v.api.revealFile(input.folder,input.file);`,{folder:'\\\\localhost\\'+folder[0]+'$'+folder.slice(2),file:'selected \u03bb file.txt'});for(let n=0;n<12;n++){uncSelected=windows().some(w=>w.selected.some(p=>p.toLowerCase()===unc.toLowerCase()));if(uncSelected)break;await sleep(150);}if(!uncSelected)throw new Error('Explorer did not select the UNC item');}
   return {selected:true,uncAvailable:available,uncSelected};
  }finally{for(const w of windows()){if(!before.has(w.hwnd)&&w.folder.toLowerCase().endsWith(folder.slice(2).toLowerCase()))powershell(`$shell=New-Object -ComObject Shell.Application;foreach($w in $shell.Windows()){if([long]$w.HWND -eq ${w.hwnd}){$w.Quit()}}`);}}
 });
 await check('O035 native superseded reads and closed tab cleanup',async()=>{
  const realGit=tools.git;const wrapper=join(run,'slow-git.cmd'), marker=join(run,'read-ready'),late=join(run,'read-late');
  writeFileSync(wrapper,`@echo off\r\nif "%~1"=="log" goto slow\r\nif "%~1"=="--literal-pathspecs" if "%~2"=="log" goto slow\r\n"${realGit}" %*\r\nexit /b %errorlevel%\r\n:slow\r\necho ready>"${marker}"\r\n"${tools.ping}" -n 31 127.0.0.1 >nul\r\necho leaked>"${late}"\r\n`);
  await evaluate(`await v.invoke('save_settings',{settings:{settings:{gitPath:input.gitPath,sshUseAgent:true}}});window.controllers=Array.from({length:12},()=>new AbortController());window.reads=controllers.map(c=>v.api.openRepo(path,500,undefined,c.signal).then(()=>({ok:true}),error=>({ok:false,error:String(error)})));`,{gitPath:wrapper});
  for(let i=0;i<200&&!existsSync(marker);i++)await sleep(25);if(!existsSync(marker))throw new Error('Slow native Git helper did not start');
  const result=await evaluate(`const start=performance.now();controllers.forEach(c=>c.abort());const readsDone=await Promise.all(reads);return {cancelMs:performance.now()-start,rejected:readsDone.filter(r=>!r.ok).length};`);
  await until('(await v.invoke("desktop_validation_state")).read_slots===0','read slots released');
  await evaluate(`v.store.getState().setUi(tabId,{history:{file:'tracked.txt',mode:'history',sha:null}});`);await sleep(250);
  const close=await evaluate(`const start=performance.now();v.store.getState().closeTab(tabId);return {closedAt:start};`);await until('(await v.invoke("desktop_validation_state")).read_slots===0','closed tab read release');
  const closeMs=await evaluate(`return performance.now()-input.closedAt;`,{closedAt:close.closedAt});
  await evaluate(`await v.invoke('save_settings',{settings:{settings:{gitPath:input.gitPath,sshUseAgent:true}}});`,{gitPath:tools.git});
  if(existsSync(late))throw new Error('Cancelled helper survived to write');if(result.rejected!==12)throw new Error('A cancelled slow read succeeded');return {...result,closeMs};
 });
 const violations=await evaluate('return violations;');report.violations=violations;if(violations.length)throw new Error('Content-policy violations: '+JSON.stringify(violations));
 if(report.exceptions.length)throw new Error('Unexpected renderer exceptions: '+JSON.stringify(report.exceptions));
 report.heap=await send('Runtime.getHeapUsage');
}catch(error){report.error=String(error);process.exitCode=1;}
finally{
 if(send){try{await evaluate(`const state=await v.invoke('desktop_validation_state');for(const [id] of state.terminals)await v.invoke('term_close',{id});`);}catch{}}
 socket?.close();child.kill();await sleep(500);
 writeFileSync(join(run,'native.log'),log);writeFileSync(join(run,'report.json'),JSON.stringify(report,null,2));writeFileSync(join(artifact,`desktop-${tag}-report.json`),JSON.stringify(report,null,2));
 console.log(JSON.stringify({report:join(run,'report.json'),checks:report.checks.map(c=>({id:c.id,status:c.status})),error:report.error},null,2));
}
