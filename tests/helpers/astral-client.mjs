import {spawn} from 'node:child_process';
import assert from 'node:assert/strict';
export async function client(binary,workspace,env={}){
 const child=spawn(binary,['--workspace',workspace,'mcp','--headless'],{windowsHide:true,env:{...process.env,...env},stdio:['pipe','pipe','pipe']});let sequence=0,buffer='',stderr='';const pending=new Map();child.stdout.setEncoding('utf8');child.stderr.on('data',b=>stderr+=b);
 child.stdout.on('data',b=>{buffer+=b;let n;while((n=buffer.indexOf('\n'))>=0){const line=buffer.slice(0,n);buffer=buffer.slice(n+1);if(!line.trim())continue;try{const m=JSON.parse(line);const p=pending.get(m.id);if(p){pending.delete(m.id);clearTimeout(p.timer);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);}}catch(e){for(const p of pending.values())p.reject(e);}}});
 child.on('error',e=>{for(const p of pending.values())p.reject(e);});child.on('exit',code=>{for(const p of pending.values()){clearTimeout(p.timer);p.reject(Error(`Astral exited ${code}: ${stderr}`));}pending.clear();});
 function rpc(method,params={}){const id=++sequence;return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{pending.delete(id);reject(Error(`Timed out: ${method}`));},30000);pending.set(id,{resolve,reject,timer});child.stdin.write(JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n');});}
 async function tool(name,args={}){const r=await rpc('tools/call',{name,arguments:args});assert.ok(!r.isError,JSON.stringify(r));return r.structuredContent??JSON.parse(r.content.find(c=>c.type==='text').text);}
 async function job(value){for(let i=0;i<4000;i++){const result=await tool('job_status',{job:value.job});if(result.state==='completed')return result;assert.notEqual(result.state,'failed',JSON.stringify(result));assert.notEqual(result.state,'cancelled');await new Promise(r=>setTimeout(r,50));}throw Error('Job timed out');}
 await rpc('initialize',{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'Astral release acceptance',version:'0.15.0'}});child.stdin.write(JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})+'\n');
 return{rpc,tool,job,close(){child.stdin.end();setTimeout(()=>child.kill(),2000).unref();},pid:child.pid};
}
