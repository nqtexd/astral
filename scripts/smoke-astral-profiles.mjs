import {spawn} from 'node:child_process';
import {mkdir,writeFile} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
const root=path.resolve(import.meta.dirname,'..'),workspace=process.env.ASTRAL_PROFILE_WORKSPACE??path.join(root,'artifacts','astral-profiles');
const live=process.env.ASTRAL_PROFILE_LIVE==='1';
await mkdir(workspace,{recursive:true});
const executable=process.env.ASTRAL_BINARY??path.join(root,'target','debug','astral.exe');
async function connect(profile){
  const child=spawn(executable,['--workspace',workspace,'mcp',...(live?[]:['--headless']),'--profile',profile],{windowsHide:true,stdio:['pipe','pipe','pipe']});
  let sequence=0,buffer='',stderr='';const pending=new Map();
  child.stderr.on('data',b=>stderr+=b);child.stdout.setEncoding('utf8');
  child.stdout.on('data',b=>{buffer+=b;let n;while((n=buffer.indexOf('\n'))>=0){const line=buffer.slice(0,n);buffer=buffer.slice(n+1);if(!line.trim())continue;const m=JSON.parse(line),p=pending.get(m.id);if(p){pending.delete(m.id);clearTimeout(p.timer);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);}}});
  child.on('exit',code=>{for(const p of pending.values()){clearTimeout(p.timer);p.reject(Error(`${code}: ${stderr}`));}pending.clear();});
  const rpc=(method,params={})=>new Promise((resolve,reject)=>{const id=++sequence;const timer=setTimeout(()=>{pending.delete(id);reject(Error('RPC timeout'));},30000);pending.set(id,{resolve,reject,timer});child.stdin.write(JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n');});
  await rpc('initialize',{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'profile-test',version:'1'}});
  child.stdin.write(JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})+'\n');
  return {rpc,close:()=>child.stdin.end(),tool:async(name,args={})=>{const r=await rpc('tools/call',{name,arguments:args});assert.ok(!r.isError,JSON.stringify(r));return r.structuredContent??JSON.parse(r.content.find(c=>c.type==='text').text);}};
}
const projects=await connect('projects');let file,createdProject;
try{
  const names=(await projects.rpc('tools/list')).tools.map(t=>t.name);
  assert.equal(names.length,11);assert.ok(names.includes("storyboard"));assert.ok(names.includes('create_managed_project'));assert.ok(!names.includes('animate_character'));
  const p=await projects.tool('create_managed_project',{name:'Profile handoff',width:640,height:360,fps:24,duration:120,path:`${Date.now()}/Film.astral`});file=p.path;createdProject=p;
  const saved=await projects.tool('list_saved_projects');assert.ok(saved.files.length>0);
  if(!live)await projects.tool('close_project',{project:p.project,expected_revision:p.revision});
}finally{projects.close();}
const animation=await connect('animation');
try{
  const names=(await animation.rpc('tools/list')).tools.map(t=>t.name);
  assert.ok(names.includes('animate_character'));assert.ok(names.includes('add_voice'));assert.ok(names.includes('motion_tools'));assert.ok(names.includes('storyboard'));assert.ok(names.includes('review_animation'));assert.ok(!names.includes('create_managed_project'));
  const relative=file;
  const p=await animation.tool('open_project',{path:relative});
  if(live){assert.equal(p.project,createdProject.project);assert.equal(p.revision,createdProject.revision);}
  const c=await animation.tool('create_character',{project:p.project,expected_revision:p.revision,name:'Agent figure',position:{x:320,y:220}});
  const a=await animation.tool('animate_character',{project:p.project,expected_revision:c.revision,character:c.created_ids[0],action:'jump',start:0,end:47});
  assert.equal(a.revision,c.revision+1);
  const motion=await animation.rpc('tools/call',{name:'motion_tools',arguments:{operation:'edit',project:p.project,expected_revision:a.revision,apply:false,preview_frame:0,tool:{task:'smart_pose',character:c.created_ids[0],frame:0,joint:'right_hand',target:{x:375,y:185},style:'natural'}}});
  assert.ok(!motion.isError,JSON.stringify(motion));assert.equal(motion.content.filter(c=>c.type==='image').length,2);
  assert.equal((await animation.tool('inspect_project',{project:p.project})).document.revision,a.revision);

  await animation.tool('save_project',{project:p.project,path:'Film.astral'});
  const report={project_profile_tools:11,animation_profile_tools:names.length,headless_saved_file_handoff:!live,live_shared_document:live,motion_tool_preview:true};
  await writeFile(path.join(workspace,'report.json'),JSON.stringify(report,null,2));console.log(report);
}finally{animation.close();}
