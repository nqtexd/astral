import {spawn} from 'node:child_process';
import {writeFile,readFile,mkdir} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
const root=path.resolve(import.meta.dirname,'..');
const workspace=path.join(root,'artifacts','astral-artwork-check');
const executable=process.env.ASTRAL_BINARY??path.join(root,'target','release','astral.exe');
await mkdir(workspace,{recursive:true});
const child=spawn(executable,['--workspace',workspace,'mcp','--headless'],{windowsHide:true,env:{...process.env,ASTRAL_FFMPEG:'encoder-deliberately-unavailable'},stdio:['pipe','pipe','pipe']});
child.stdout.setEncoding('utf8');child.stderr.setEncoding('utf8');
let next=0,buffer='',stderr='';const pending=new Map();
child.stderr.on('data',b=>stderr+=b);
child.stdout.on('data',b=>{buffer+=b;let newline;while((newline=buffer.indexOf('\n'))>=0){const line=buffer.slice(0,newline);buffer=buffer.slice(newline+1);if(!line.trim())continue;let message;try{message=JSON.parse(line)}catch(e){for(const p of pending.values())p.reject(e);continue}if(message.id!=null){const p=pending.get(message.id);if(p){pending.delete(message.id);clearTimeout(p.timer);message.error?p.reject(new Error(JSON.stringify(message.error))):p.resolve(message.result);}}}});
child.on('exit',code=>{for(const p of pending.values()){clearTimeout(p.timer);p.reject(new Error(`Astral exited ${code}: ${stderr}`));}pending.clear();});
function rpc(method,params={}){const id=++next;return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{pending.delete(id);reject(new Error(`Timed out: ${method}`));},30000);pending.set(id,{resolve,reject,timer});child.stdin.write(JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n');});}
async function tool(name,args={}){const result=await rpc('tools/call',{name,arguments:args});assert.ok(!result.isError,JSON.stringify(result));return result.structuredContent??JSON.parse(result.content.find(c=>c.type==='text').text);}
async function preview(project,frame){const r=await rpc('tools/call',{name:'render_preview',arguments:{project,frame,width:128,height:64}});assert.ok(!r.isError,JSON.stringify(r));const image=r.content.find(c=>c.type==='image');assert.ok(image);return image.data;}
async function finished(value){for(let i=0;i<600;i++){const result=await tool('job_status',{job:value.job});if(result.state==='completed')return result;assert.notEqual(result.state,'failed',JSON.stringify(result));assert.notEqual(result.state,'cancelled');await new Promise(r=>setTimeout(r,50));}throw Error('Artwork job timed out');}
function crc32(bytes){let crc=0xffffffff;for(const byte of bytes){crc^=byte;for(let i=0;i<8;i++)crc=(crc>>>1)^((crc&1)?0xedb88320:0);}return(crc^0xffffffff)>>>0;}
function chunk(type,bytes){const name=Buffer.from(type),out=Buffer.alloc(bytes.length+12);out.writeUInt32BE(bytes.length);name.copy(out,4);bytes.copy(out,8);out.writeUInt32BE(crc32(Buffer.concat([name,bytes])),bytes.length+8);return out;}
async function png(color){const{deflateSync}=await import('node:zlib');const header=Buffer.alloc(13);header.writeUInt32BE(16,0);header.writeUInt32BE(16,4);header[8]=8;header[9]=6;const rows=Buffer.alloc(16*65);for(let y=0;y<16;y++)for(let x=0;x<16;x++)Buffer.from(color).copy(rows,y*65+1+x*4);return Buffer.concat([Buffer.from('89504e470d0a1a0a','hex'),chunk('IHDR',header),chunk('IDAT',deflateSync(rows)),chunk('IEND',Buffer.alloc(0))]);}
try{
 await rpc('initialize',{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'Artwork acceptance',version:'1'}});child.stdin.write(JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})+'\n');
 const listed=await rpc('tools/list');assert.equal(listed.tools.find(t=>t.name==='artwork_tools').inputSchema.type,'object');
 const project=(await tool('create_project',{name:'Artwork transport',width:128,height:64,fps:24,duration:24})).project;
 await writeFile(path.join(workspace,'red.png'),await png([255,0,0,255]));await writeFile(path.join(workspace,'blue.png'),await png([0,0,255,255]));
 await finished(await tool('artwork_tools',{operation:'import',project,expected_revision:0,paths:['red.png','blue.png'],start:0,source_fps:24,looping:true}));
 let p=(await tool('inspect_project',{project})).document;assert.equal(p.rasters.length,1);assert.equal(p.bitmaps.length,2);assert.ok(p.bitmaps.every(b=>!('png'in b)&&b.encoded_bytes>0));
 const before0=await preview(project,0);const before1=await preview(project,1);
 await tool('artwork_tools',{operation:'paint',project,expected_revision:p.revision,layer:p.rasters[0].id,frame:0,points:[{x:8,y:8}],radius:6,color:[0,255,0,255],erase:false});
 assert.notEqual(await preview(project,0),before0);assert.equal(await preview(project,1),before1);
 p=(await tool('inspect_project',{project})).document;
 const stale=await rpc('tools/call',{name:'artwork_tools',arguments:{operation:'paint',project,expected_revision:0,layer:p.rasters[0].id,frame:0,points:[{x:8,y:8}],radius:1,color:[0,0,0,255]}});assert.equal(stale.isError,true);assert.equal((await tool('inspect_project',{project})).document.revision,p.revision);
 const exported=await finished(await tool('export_media',{project,spec:{path:'native-sized.png',renderer:'cpu',width:256,height:128,start:0,end:0}}));assert.equal(exported.metrics.renderer,'cpu');assert.equal(exported.plan.width,256);assert.equal((await readFile(path.join(workspace,'native-sized.png'))).subarray(0,8).toString('hex'),'89504e470d0a1a0a');
 await tool('save_project',{project,path:'artwork.astral'});const reopened=await tool('open_project',{path:'artwork.astral'});assert.equal(await preview(reopened.project,1),before1);
 const report={passed:true,tool:'artwork_tools',schema_object:true,embedded_images_hidden_from_inspection:true,independent_painted_frame:true,stale_revision_rollback:true,native_sized_png_without_ffmpeg:true,save_reopen:true};await writeFile(path.join(workspace,'report.json'),JSON.stringify(report,null,2));console.log(report);
}finally{child.stdin.end();}

async function waitJob(job){let result;do{await new Promise(r=>setTimeout(r,100));result=await tool('job_status',{job});}while(!['completed','failed','cancelled'].includes(result.state));assert.equal(result.state,'completed',JSON.stringify(result));return result;}
