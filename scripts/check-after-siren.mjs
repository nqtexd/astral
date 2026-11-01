import {spawn} from 'node:child_process';
import {writeFile,readFile} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {randomUUID,createHash} from 'node:crypto';
const root=path.resolve(import.meta.dirname,'..');
const workspace=path.join(root,'productions','after-the-siren');
const executable=process.env.ASTRAL_BINARY??path.join(root,'release','Astral-0.9.1-windows-x64','astral.exe');
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
 for(const name of ['storyboard','review_animation','motion_tools','scene_interactions'])assert.ok(listed.tools.some(t=>t.name===name));
 const opened=await tool('open_project',{path:'AFTER-THE-SIREN.astral'});const project=opened.project;
 const doc=(await tool('inspect_project',{project})).document;
 const declared=await tool('scene_interactions',{project,expected_revision:doc.revision});
 assert.equal(declared.cues.length,5);
 const staleInteraction=await rpc('tools/call',{name:'scene_interactions',arguments:{project,expected_revision:doc.revision-1,cues:declared.cues}});assert.equal(staleInteraction.isError,true);
 const board=await tool('storyboard',{project,expected_revision:doc.revision});
 assert.equal(board.board.shots.length,30);assert.equal(board.board.characters.length,8);
 const stale=await rpc('tools/call',{name:'storyboard',arguments:{project,expected_revision:doc.revision-1,board:board.board}});assert.equal(stale.isError,true);
 const reports=[];
 for(const shot of board.board.shots){
  const report=await tool('review_animation',{project,start:shot.start,end:shot.end,step:1});
  reports.push({shot:shot.title,start:shot.start,contacts:report.contacts,interactions:report.interactions,overlaps:report.overlaps,samples:report.sampled_frames});
 }
 const missed=reports.flatMap(r=>r.contacts.filter(c=>!c.passed).map(c=>({shot:r.shot,...c})));
 const overlaps=reports.flatMap(r=>r.overlaps.map(c=>({shot:r.shot,...c})));
 const failed_interactions=reports.flatMap(r=>r.interactions.filter(i=>!i.passed).map(i=>({shot:r.shot,...i})));
 const summary={project_sha256:createHash('sha256').update(await readFile(path.join(workspace,'AFTER-THE-SIREN.astral'))).digest('hex'),sample_step:1,tools:listed.tools.length,shots:reports.length,contacts:reports.reduce((n,r)=>n+r.contacts.length,0),interactions:declared.cues.length,failed_interactions,missed_contacts:missed,overlaps,reports};
 await writeFile(path.join(workspace,'MOTION-REVIEW.json'),JSON.stringify(summary,null,2));
 console.log(JSON.stringify({tools:summary.tools,shots:summary.shots,contacts:summary.contacts,interactions:summary.interactions,failed_interactions,missed:missed.length,overlaps},null,2));
 assert.equal(missed.length,0,'Contact misses remain');
 assert.equal(failed_interactions.length,0,JSON.stringify(failed_interactions));
 assert.equal(overlaps.length,0,'Torso overlaps remain');
 // Persisted board mutations still reject stale revisions and support normal undo.
 const changed=structuredClone(board.board);changed.logline+=' QA transaction.';
 const update=await tool('storyboard',{project,expected_revision:doc.revision,board:changed});
 assert.equal(update.revision,doc.revision+1);
 const undo=await tool('history',{project,expected_revision:update.revision,redo:false});
 assert.equal((await tool('storyboard',{project,expected_revision:undo.revision})).board.logline,board.board.logline);
 const interactionEdit=await tool('scene_interactions',{project,expected_revision:undo.revision,cues:declared.cues});
 assert.equal(interactionEdit.revision,undo.revision+1);
 assert.deepEqual(interactionEdit.cues,declared.cues);
 const interactionUndo=await tool('history',{project,expected_revision:interactionEdit.revision,redo:false});
 assert.deepEqual((await tool('scene_interactions',{project,expected_revision:interactionUndo.revision})).cues,declared.cues);
 // A run without manual travel must move at a physical pace, at any FPS.
 const auto=await tool('create_project',{name:'Natural running acceptance',width:1280,height:720,fps:24,duration:60});
 const figure=await tool('create_character',{project:auto.project,expected_revision:auto.revision,name:'Runner',position:{x:200,y:520}});
 const action=await tool('animate_character',{project:auto.project,expected_revision:figure.revision,character:figure.created_ids[0],action:'run',start:0,end:47});
 const result=(await tool('inspect_project',{project:auto.project,frame:47})).document;
 assert.ok(result.actions[0].distance>500,'Run defaulted to walking speed or no travel');
 console.log('Storyboard transaction, per-frame contact review, and natural-pace MCP run passed.');
} finally {child.stdin.end();child.kill();}
