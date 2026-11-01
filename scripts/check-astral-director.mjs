import {spawn} from 'node:child_process';
import {writeFile,readFile,mkdir} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {randomUUID,createHash} from 'node:crypto';
const root=path.resolve(import.meta.dirname,'..');
const workspace=path.join(root,'artifacts','astral-010');
const executable=process.env.ASTRAL_BINARY??path.join(root,'release','Astral-0.10.0-windows-x64','astral.exe');
await mkdir(workspace,{recursive:true});
const child=spawn(executable,['--workspace',workspace,'mcp','--headless'],{windowsHide:true,stdio:['pipe','pipe','pipe']});
child.stdout.setEncoding('utf8');child.stderr.setEncoding('utf8');
let next=0,buffer='',stderr='';const pending=new Map();
child.stderr.on('data',b=>stderr+=b);
child.stdout.on('data',b=>{buffer+=b;let newline;while((newline=buffer.indexOf('\n'))>=0){const line=buffer.slice(0,newline);buffer=buffer.slice(newline+1);if(!line.trim())continue;let message;try{message=JSON.parse(line)}catch(e){for(const p of pending.values())p.reject(e);continue}if(message.id!=null){const p=pending.get(message.id);if(p){pending.delete(message.id);clearTimeout(p.timer);message.error?p.reject(new Error(JSON.stringify(message.error))):p.resolve(message.result);}}}});
child.on('exit',code=>{for(const p of pending.values()){clearTimeout(p.timer);p.reject(new Error(`Astral exited ${code}: ${stderr}`));}pending.clear();});
function rpc(method,params={}){const id=++next;return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{pending.delete(id);reject(new Error(`Timed out: ${method}`));},30000);pending.set(id,{resolve,reject,timer});child.stdin.write(JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n');});}
async function tool(name,args={}){const result=await rpc('tools/call',{name,arguments:args});assert.ok(!result.isError,JSON.stringify(result));return result.structuredContent??JSON.parse(result.content.find(c=>c.type==='text').text);}

try {
 await rpc('initialize',{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'Astral manual/director acceptance',version:'1'}});
 child.stdin.write(JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})+'\n');
 const listed=await rpc('tools/list');
 for(const name of ['draw_frame','auto_animate','scene_interactions','export_media'])assert.ok(listed.tools.some(t=>t.name===name));
 const manual=await tool('create_project',{name:'Independent manual frames',width:960,height:540,fps:24,duration:24});
 const layer=randomUUID();
 const added=await tool('apply_commands',{project:manual.project,expected_revision:0,commands:[{type:'add_layer',layer:{id:layer,name:'Ink',visible:true,locked:false,opacity:1}}]});
 const stroke=x=>({points:[{x,y:150},{x:x+50,y:220}],color:[20,30,40,255],width:5});
 const first=await tool('draw_frame',{project:manual.project,expected_revision:added.revision,layer,frame:0,strokes:[stroke(100)]});
 const second=await tool('draw_frame',{project:manual.project,expected_revision:first.revision,layer,frame:1,strokes:[stroke(180)]});
 const doc=(await tool('inspect_project',{project:manual.project})).document;
 assert.equal(doc.cels.find(c=>c.frame===0).strokes[0].points[0].x,100);
 assert.equal(doc.cels.find(c=>c.frame===1).strokes[0].points[0].x,180);
 assert.equal(doc.cels.find(c=>c.frame===2).strokes.length,0);
 const stale=await rpc('tools/call',{name:'draw_frame',arguments:{project:manual.project,expected_revision:first.revision,layer,frame:0,strokes:[]}});assert.equal(stale.isError,true);
 await tool('save_project',{project:manual.project,expected_revision:second.revision,path:'manual-frames.astral'});
 for(const frame of [0,1,2])await tool('render_preview',{project:manual.project,frame,width:480,height:270,path:`manual-${frame}.png`});

 const auto=await tool('create_project',{name:'Offline director acceptance',width:960,height:540,fps:24,duration:144});
 const spec={mode:'create',start:0,end:143,seed:3,max_repair_attempts:2};
 const proposed=await tool('auto_animate',{project:auto.project,expected_revision:0,spec});
 assert.equal(proposed.applied,false);assert.equal(proposed.quality_gate_passed,true);
 assert.equal((await tool('inspect_project',{project:auto.project})).document.characters.length,0);
 const committed=await tool('apply_commands',{project:auto.project,expected_revision:0,commands:proposed.plan.commands});
 assert.equal(committed.revision,1);
 const reviewed=await tool('auto_animate',{project:auto.project,expected_revision:1,spec:{mode:'inspect',start:0,end:143}});
 assert.equal(reviewed.plan.commands.length,0);assert.equal(reviewed.quality_gate_passed,true);
 const board=await tool('storyboard',{project:auto.project,expected_revision:1});assert.equal(board.board.shots.length,4);
 await tool('render_preview',{project:auto.project,frame:reviewed.review.contacts[0].cue.frame,width:960,height:540,path:'auto-contact.png'});
 await tool('save_project',{project:auto.project,expected_revision:1,path:'auto-spar.astral'});
 const movie=await tool('export_media',{project:auto.project,spec:{path:'auto-spar.mp4',width:960,height:540,fps:24,hardware:true,start:0,end:143}});
 let job;
 for(let i=0;i<600;i++) {
  job=await tool('job_status',{job:movie.job});
  if(['completed','failed','cancelled'].includes(job.state))break;
  await new Promise(resolve=>setTimeout(resolve,100));
 }
 assert.equal(job.state,'completed',JSON.stringify(job));
 const undone=await tool('history',{project:auto.project,expected_revision:1,redo:false});
 assert.equal((await tool('inspect_project',{project:auto.project})).document.characters.length,0);
 const applied=await tool('auto_animate',{project:auto.project,expected_revision:undone.revision,spec,apply:true});
 assert.equal(applied.applied,true);assert.equal(applied.revision,undone.revision+1);
 const report={tools:listed.tools.length,manual_frames_independent:true,stale_rejected:true,proposal_non_mutating:true,
  storyboard_shots:4,contacts:reviewed.review.contacts.length,quality_gate_passed:true,one_step_undo:true,
  explicit_apply_passed:true,export:job,warning:'Offline seeded procedural director; no embedded LLM or artistic quality score.'};
 await writeFile(path.join(workspace,'ACCEPTANCE.json'),JSON.stringify(report,null,2));
 console.log(JSON.stringify(report,null,2));
} finally {child.stdin.end();child.kill();}


