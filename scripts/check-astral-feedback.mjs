import {spawn} from 'node:child_process';
import {writeFile,readFile,mkdir} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {randomUUID,createHash} from 'node:crypto';
const root=path.resolve(import.meta.dirname,'..');
const workspace=path.join(root,'artifacts','astral-feedback-check');
const executable=process.env.ASTRAL_BINARY??path.join(root,'target','release','astral.exe');
await mkdir(workspace,{recursive:true});
const child=spawn(executable,['--workspace',workspace,'mcp','--headless'],{windowsHide:true,stdio:['pipe','pipe','pipe']});
child.stdout.setEncoding('utf8');child.stderr.setEncoding('utf8');
let next=0,buffer='',stderr='';const pending=new Map();
child.stderr.on('data',b=>stderr+=b);
child.stdout.on('data',b=>{buffer+=b;let newline;while((newline=buffer.indexOf('\n'))>=0){const line=buffer.slice(0,newline);buffer=buffer.slice(newline+1);if(!line.trim())continue;let message;try{message=JSON.parse(line)}catch(e){for(const p of pending.values())p.reject(e);continue}if(message.id!=null){const p=pending.get(message.id);if(p){pending.delete(message.id);clearTimeout(p.timer);message.error?p.reject(new Error(JSON.stringify(message.error))):p.resolve(message.result);}}}});
child.on('exit',code=>{for(const p of pending.values()){clearTimeout(p.timer);p.reject(new Error(`Astral exited ${code}: ${stderr}`));}pending.clear();});
function rpc(method,params={}){const id=++next;return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{pending.delete(id);reject(new Error(`Timed out: ${method}`));},30000);pending.set(id,{resolve,reject,timer});child.stdin.write(JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n');});}
async function tool(name,args={}){const result=await rpc('tools/call',{name,arguments:args});assert.ok(!result.isError,JSON.stringify(result));return result.structuredContent??JSON.parse(result.content.find(c=>c.type==='text').text);}

async function waitJob(job){let result;do{await new Promise(r=>setTimeout(r,100));result=await tool('job_status',{job});}while(!['completed','failed','cancelled'].includes(result.state));assert.equal(result.state,'completed',JSON.stringify(result));return result;}
try {
 await rpc('initialize',{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'Feedback acceptance',version:'1'}});child.stdin.write(JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})+'\n');
 assert.ok((await rpc('tools/list')).tools.some(t=>t.name==='assemble_video'));
 const p=await tool('create_project',{name:'Readable acting',width:960,height:540,fps:24,duration:72});
 let receipt=await tool('apply_commands',{project:p.project,expected_revision:0,commands:[{type:'add_character',name:'Actor',position:{x:480,y:350},color:[41,47,60,255]}]});
 let doc=(await tool('inspect_project',{project:p.project})).document;const actor=doc.characters[0];
 const keys=[0,18,36,54,71].map((frame,i)=>{const pose=structuredClone(actor.keys[0].pose);pose.root.x+=Math.sin(i*0.9)*7;const e=actor.joints.findIndex(j=>j.name==='right_elbow'),h=actor.joints.findIndex(j=>j.name==='right_hand');const length=Math.hypot(pose.joints[h].x-pose.joints[e].x,pose.joints[h].y-pose.joints[e].y);const angle=[1.15,0.45,-0.5,-0.9,0.6][i];pose.joints[h]={x:pose.joints[e].x+Math.cos(angle)*length,y:pose.joints[e].y+Math.sin(angle)*length};return {frame,pose,easing:'smooth'};});
 const commands=[{type:'set_pose_keys',character:actor.id,keys},...['left_foot','right_foot'].map(name=>{const joint=actor.joints.findIndex(j=>j.name===name),v=actor.keys[0].pose.joints[joint];return{type:'set_contact',character:actor.id,contact:{start:0,end:71,joint,target:{x:480+v.x,y:350+v.y}}};})];
 receipt=await tool('apply_commands',{project:p.project,expected_revision:receipt.revision,commands});
 receipt=await tool('add_dialogue',{project:p.project,expected_revision:receipt.revision,text:'We can keep the words editable.',speaker:'Actor',start:0,end:71,position:{x:420,y:210},size:{x:330,y:130},font_size:24,style:'caption',color:[24,27,33,255],text_color:[238,241,247,255],appearance:{border_color:[63,69,79,255],border_width:1,corner_radius:12,speaker_color:[190,198,212,255]}});
 doc=(await tool('inspect_project',{project:p.project})).document;const dialogue=doc.dialogues[0];
 const review=await tool('review_animation',{project:p.project,start:0,end:71,step:1});assert.ok(review.layout.length>0);
 const preview=await tool('render_preview',{project:p.project,frame:18,width:640,height:360,path:'before.png'});assert.ok(preview.layout_review.issues.length>0);
 const proposal=await tool('motion_tools',{operation:'edit',project:p.project,expected_revision:doc.revision,tool:{task:'layout_dialogue',dialogue:dialogue.id},apply:false,preview_frame:18});
 assert.deepEqual((await tool('inspect_project',{project:p.project})).document.dialogues,doc.dialogues);
 receipt=await tool('apply_commands',{project:p.project,expected_revision:doc.revision,commands:proposal.plan.commands});
 assert.equal((await tool('review_animation',{project:p.project,start:0,end:71,step:1})).layout.length,0);
 await tool('save_project',{project:p.project,path:'acting-poses.astral'});
 await waitJob((await tool('export_media',{project:p.project,spec:{path:'acting-poses.mp4',width:640,height:360,quality:'draft'}})).job);
 const flow=await tool('motion_tools',{operation:'edit',project:p.project,expected_revision:receipt.revision,tool:{task:'flow_motion',character:actor.id,start:0,end:71},apply:false,preview_frame:18});
 receipt=await tool('apply_commands',{project:p.project,expected_revision:receipt.revision,commands:flow.plan.commands});
 doc=(await tool('inspect_project',{project:p.project})).document;assert.ok(doc.characters[0].keys.every(k=>k.easing==='flow'));assert.equal(doc.dialogues[0].text,dialogue.text);
 await tool('save_project',{project:p.project,path:'acting-flow.astral'});
 await waitJob((await tool('export_media',{project:p.project,spec:{path:'acting-flow.mp4',width:640,height:360,quality:'draft'}})).job);
 await tool('render_preview',{project:p.project,frame:18,width:640,height:360,path:'after.png'});
 const png=await readFile(path.join(workspace,'after.png'));await tool('render_preview',{project:p.project,frame:36,width:640,height:360,path:'after.png'});assert.notDeepEqual(png,await readFile(path.join(workspace,'after.png')));
 const assembled=await waitJob((await tool('assemble_video',{project:p.project,spec:{path:'finished.mp4',width:640,height:360,fps:24,clips:[{path:'acting-poses.mp4',start_seconds:0.25,end_seconds:1.25,audio:false},{path:'acting-flow.mp4',start_seconds:1.25,end_seconds:2.5,audio:false}]}})).job);
 assert.equal(assembled.duration_seconds,2.25);
 // A bad audio declaration must fail without replacing the completed output.
 const digest=createHash('sha256').update(await readFile(path.join(workspace,'finished.mp4'))).digest('hex');
 const bad=await tool('assemble_video',{project:p.project,spec:{path:'finished.mp4',width:640,height:360,fps:24,clips:[{path:'acting-poses.mp4',start_seconds:0,end_seconds:1,audio:true},{path:'acting-flow.mp4',start_seconds:0,end_seconds:1,audio:true}]}});let state;do{await new Promise(r=>setTimeout(r,100));state=await tool('job_status',{job:bad.job});}while(!['failed','completed'].includes(state.state));assert.equal(state.state,'failed');assert.equal(digest,createHash('sha256').update(await readFile(path.join(workspace,'finished.mp4'))).digest('hex'));

 const latest=(await tool('inspect_project',{project:p.project})).document;
 const renamed=await tool('rename_project',{project:p.project,expected_revision:latest.revision,name:'Managed acting test'});
 await tool('save_project',{project:p.project,path:'managed.astral'});
 const activity=await tool('project_activity',{project:p.project});assert.ok(activity.entries.some(e=>e.changes.includes('name')));
 const trashed=await tool('trash_project',{project:p.project,expected_revision:renamed.revision});
 const restored=await tool('restore_project',{path:trashed.trash_path});assert.equal(restored.project,p.project);
 assert.equal((await tool('inspect_project',{project:p.project})).document.name,'Managed acting test');
 const result={passed:true,project:p.project,checks:['project rename/activity/trash/restore','MCP discovery','composition detects collisions','non-mutating placement proposal','placement fixes sampled range','editable dark dialogue','flow through pose keys','preview overwrite','trimmed shot assembly with silent audio','failed assembly preserves previous output'],assembly:assembled};await writeFile(path.join(workspace,'ACCEPTANCE.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
}finally{child.stdin.end();setTimeout(()=>child.kill(),1000).unref();}
