import {spawn} from 'node:child_process';
import {writeFile,readFile} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {randomUUID,createHash} from 'node:crypto';
const root=path.resolve(import.meta.dirname,'..');
const workspace=path.join(root,'productions','last-bell');
const executable=process.env.ASTRAL_BINARY??path.join(root,'release','Astral-0.8.0-windows-x64','astral.exe');
const child=spawn(executable,['--workspace',workspace,'mcp','--headless'],{windowsHide:true,stdio:['pipe','pipe','pipe']});
child.stdout.setEncoding('utf8');child.stderr.setEncoding('utf8');
let next=0,buffer='',stderr='';const pending=new Map();
child.stderr.on('data',b=>stderr+=b);
child.stdout.on('data',b=>{buffer+=b;let newline;while((newline=buffer.indexOf('\n'))>=0){const line=buffer.slice(0,newline);buffer=buffer.slice(newline+1);if(!line.trim())continue;let message;try{message=JSON.parse(line)}catch(e){for(const p of pending.values())p.reject(e);continue}if(message.id!=null){const p=pending.get(message.id);if(p){pending.delete(message.id);clearTimeout(p.timer);message.error?p.reject(new Error(JSON.stringify(message.error))):p.resolve(message.result);}}}});
child.on('exit',code=>{for(const p of pending.values()){clearTimeout(p.timer);p.reject(new Error(`Astral exited ${code}: ${stderr}`));}pending.clear();});
function rpc(method,params={}){const id=++next;return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{pending.delete(id);reject(new Error(`Timed out: ${method}`));},30000);pending.set(id,{resolve,reject,timer});child.stdin.write(JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n');});}
async function tool(name,args={}){const result=await rpc('tools/call',{name,arguments:args});assert.ok(!result.isError,JSON.stringify(result));return result.structuredContent??JSON.parse(result.content.find(c=>c.type==='text').text);}


try {
 await rpc('initialize',{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'storyboard-contact-QA',version:'1.0'}});
 child.stdin.write(JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})+'\n');
 const listed=await rpc('tools/list');
 for(const name of ['storyboard','review_animation','motion_tools'])assert.ok(listed.tools.some(t=>t.name===name));
 const opened=await tool('open_project',{path:'LAST-BELL.astral'});const project=opened.project;
 const doc=(await tool('inspect_project',{project})).document;
 const board=await tool('storyboard',{project,expected_revision:doc.revision});
 assert.equal(board.board.shots.length,35);assert.equal(board.board.characters.length,16);
 const stale=await rpc('tools/call',{name:'storyboard',arguments:{project,expected_revision:doc.revision-1,board:board.board}});assert.equal(stale.isError,true);
 const reports=[];
 for(const shot of board.board.shots){
  const report=await tool('review_animation',{project,start:shot.start,end:shot.end,step:1});
  reports.push({shot:shot.title,start:shot.start,contacts:report.contacts,overlaps:report.overlaps,samples:report.sampled_frames});
 }
 const missed=reports.flatMap(r=>r.contacts.filter(c=>!c.passed).map(c=>({shot:r.shot,...c})));
 const overlaps=reports.flatMap(r=>r.overlaps.map(c=>({shot:r.shot,...c})));
 const summary={project_sha256:createHash('sha256').update(await readFile(path.join(workspace,'LAST-BELL.astral'))).digest('hex'),sample_step:1,tools:listed.tools.length,shots:reports.length,contacts:reports.reduce((n,r)=>n+r.contacts.length,0),missed_contacts:missed,overlaps,reports};
 await writeFile(path.join(workspace,'MOTION-REVIEW.json'),JSON.stringify(summary,null,2));
 console.log(JSON.stringify({tools:summary.tools,shots:summary.shots,contacts:summary.contacts,missed:missed.length,overlaps},null,2));
 assert.equal(missed.length,0,'Contact misses remain');
 const combat=reports.filter(r=>r.contacts.length);
 assert.equal(combat.flatMap(r=>r.overlaps).length,0,'Combat overlaps remain');
 // Persisted board mutations still reject stale revisions and support normal undo.
 const changed=structuredClone(board.board);changed.logline+=' QA transaction.';
 const update=await tool('storyboard',{project,expected_revision:doc.revision,board:changed});
 assert.equal(update.revision,doc.revision+1);
 const undo=await tool('history',{project,expected_revision:update.revision,redo:false});
 assert.equal((await tool('storyboard',{project,expected_revision:undo.revision})).board.logline,board.board.logline);
 console.log('Storyboard MCP transaction/undo and film contact review passed.');
} finally {child.stdin.end();child.kill();}
