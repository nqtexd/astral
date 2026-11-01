import {spawn} from 'node:child_process';
import {writeFile,readFile,mkdir} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {randomUUID,createHash} from 'node:crypto';
const root=path.resolve(import.meta.dirname,'..');
const workspace=path.join(root,'artifacts','astral-feedback');
const executable=process.env.ASTRAL_BINARY??path.join(root,'target','release',process.platform==='win32'?'astral.exe':'astral');
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
 await rpc('initialize',{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'Feedback benchmark',version:'1'}});
 child.stdin.write(JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})+'\n');
 let p=await tool('create_project',{name:'Feedback benchmark',width:1920,height:1080,fps:30,duration:60});
 let r=await tool('apply_commands',{project:p.project,expected_revision:0,commands:[{type:'add_character',name:'Actor',position:{x:960,y:720},color:[30,35,44,255]}]});
 r=await tool('add_dialogue',{project:p.project,expected_revision:r.revision,text:'Keep the editable text clear of the actor. Motion and captions should stay readable.',start:0,end:59,position:{x:80,y:60},style:'caption',font_size:30});
 await tool('save_project',{project:p.project,path:'benchmark.astral'});
 let j=await tool('export_media',{project:p.project,spec:{path:process.env.ASTRAL_BENCH_OUT??'benchmark.mp4',quality:'draft'}});
 let status;do{await new Promise(r=>setTimeout(r,250));status=await tool('job_status',{job:j.job});}while(!['completed','failed','cancelled'].includes(status.state));
 assert.equal(status.state,'completed',JSON.stringify(status));
 await writeFile(path.join(workspace,(process.env.ASTRAL_BENCH_OUT??'benchmark.mp4')+'.json'),JSON.stringify(status,null,2));console.log(JSON.stringify(status));
} finally {child.stdin.end();setTimeout(()=>child.kill(),1000).unref();}
