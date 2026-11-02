// Real stdio MCP integration: an external client builds, revises and saves a scene.
import {spawn} from 'node:child_process';
import {mkdir,writeFile,readFile} from 'node:fs/promises';
import path from 'node:path';
import {randomUUID,createHash} from 'node:crypto';
import assert from 'node:assert/strict';
import {deflateSync,inflateSync} from 'node:zlib';
import {createStartupDocument} from '../src/startup-document.mjs';
import {featureAcceptance} from './smoke-astral-features.mjs';
import {celAcceptance} from './smoke-astral-cels.mjs';
import {assistanceAcceptance} from './smoke-astral-assistance.mjs';
import {motionAcceptance} from './smoke-astral-motion.mjs';

const root=path.resolve(import.meta.dirname,'..');
const workspace=path.join(root,'artifacts','astral-smoke');
await mkdir(workspace,{recursive:true});
const executable=process.env.ASTRAL_BINARY??path.join(root,'target','debug',process.platform==='win32'?'astral.exe':'astral');
const child=spawn(executable,['--workspace',workspace,'mcp','--headless'],{windowsHide:true,stdio:['pipe','pipe','pipe']});
child.stdout.setEncoding('utf8');child.stderr.setEncoding('utf8');
let next=0,buffer='',stderr='';const pending=new Map();
child.stderr.on('data',b=>stderr+=b);
child.stdout.on('data',b=>{buffer+=b;let newline;while((newline=buffer.indexOf('\n'))>=0){const line=buffer.slice(0,newline);buffer=buffer.slice(newline+1);if(!line.trim())continue;let message;try{message=JSON.parse(line)}catch(e){for(const p of pending.values())p.reject(e);continue}if(message.id!=null){const p=pending.get(message.id);if(p){pending.delete(message.id);clearTimeout(p.timer);message.error?p.reject(new Error(JSON.stringify(message.error))):p.resolve(message.result);}}}});
child.on('exit',code=>{for(const p of pending.values()){clearTimeout(p.timer);p.reject(new Error(`Astral exited ${code}: ${stderr}`));}pending.clear();});
function rpc(method,params={}){const id=++next;return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{pending.delete(id);reject(new Error(`Timed out: ${method}`));},30000);pending.set(id,{resolve,reject,timer});child.stdin.write(JSON.stringify({jsonrpc:'2.0',id,method,params})+'\n');});}
async function tool(name,args={}){const result=await rpc('tools/call',{name,arguments:args});assert.ok(!result.isError,JSON.stringify(result));return result.structuredContent??JSON.parse(result.content.find(c=>c.type==='text').text);}

try{
  const initialized=await rpc('initialize',{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'astral-acceptance',version:'1.0'}});
  assert.ok(initialized.capabilities.tools);
  child.stdin.write(JSON.stringify({jsonrpc:'2.0',method:'notifications/initialized'})+'\n');
  const tools=await rpc('tools/list');assert.ok(tools.tools.some(t=>t.name==='apply_commands'));assert.ok(tools.tools.length>=15);
  const destinations=await tool('list_export_presets');assert.equal(destinations.presets.length,9);assert.equal(destinations.formats.length,8);
  const p=await tool('create_project',{name:'Agent fight',width:1920,height:1080,fps:30,duration:300});
  let revision=p.revision;const project=p.project;
  async function apply(commands){const result=await tool('apply_commands',{project,expected_revision:revision,commands});revision=result.revision;return result;}
  const added=await apply([{type:'add_character',name:'Nova',position:{x:650,y:650},color:[167,139,250,255]},{type:'add_character',name:'Orion',position:{x:1250,y:650},color:[74,222,198,255]}]);
  const commands=[];
  for(let index=0;index<2;index++){
    const character=added.created_ids[index];
    for(const[kind,start,end,distance]of [['run',0,89,210],['strike',90,119,0],['recoil',120,149,0],['jump',150,209,30],['strike',210,239,0],['idle',240,299,0]])commands.push({type:'apply_action',clip:{id:randomUUID(),character,kind,start,end,intensity:1,distance,direction:index===0?1:-1,markers:{accent:Math.floor((start+end)/2)}}});
  }
  for(const frame of[104,134,224])commands.push({type:'add_effect',effect:{id:randomUUID(),name:'Impact',kind:'impact',start:frame,end:frame+10,position:{x:960,y:570},character:null,joint:null,color:[255,213,116,255],intensity:1.5,seed:frame}});
  commands.push({type:'set_camera',key:{frame:0,center:{x:960,y:540},zoom:1}},{type:'set_camera',key:{frame:104,center:{x:960,y:560},zoom:1.3}});
  await apply(commands);
  // Generate a local WAV fixture; no external services or model keys.
  const wav=Buffer.alloc(44+24000);wav.write('RIFF');wav.writeUInt32LE(wav.length-8,4);wav.write('WAVEfmt ',8);wav.writeUInt32LE(16,16);wav.writeUInt16LE(1,20);wav.writeUInt16LE(1,22);wav.writeUInt32LE(48000,24);wav.writeUInt32LE(96000,28);wav.writeUInt16LE(2,32);wav.writeUInt16LE(16,34);wav.write('data',36);wav.writeUInt32LE(24000,40);for(let i=0;i<12000;i++)wav.writeInt16LE(Math.round(Math.sin(i/48000*160*Math.PI*2)*Math.exp(-i/48000*25)*22000),44+i*2);
  await mkdir(path.join(workspace,'assets'),{recursive:true});await writeFile(path.join(workspace,'assets','impact.wav'),wav);
  const asset=await tool('import_asset',{project,expected_revision:revision,path:'assets/impact.wav',tags:['impact']});revision=asset.revision;
  await apply([104,134,224].map(start=>({type:'add_audio',clip:{id:randomUUID(),name:'Impact cue',asset:asset.created_ids[0],start,offset_seconds:0,duration_seconds:0.25,gain:0.6,fade_in:0.002,fade_out:0.03,looping:false}})));
  const before=(await tool('inspect_project',{project,frame:104})).document;
  const clip=structuredClone(before.actions[1]);clip.intensity=1.8;await apply([{type:'replace_action',clip}]);
  const after=(await tool('inspect_project',{project,frame:104})).document;
  assert.deepEqual(after.audio,before.audio);assert.deepEqual(after.effects,before.effects);assert.equal(after.actions.filter((a,i)=>JSON.stringify(a)!==JSON.stringify(before.actions[i])).length,1);
  const stale=await rpc('tools/call',{name:'apply_commands',arguments:{project,expected_revision:0,commands:[{type:'remove',id:after.characters[0].id}]}});assert.equal(stale.isError,true);
  const invalid=await rpc('tools/call',{name:'apply_commands',arguments:{project,expected_revision:revision,commands:[{type:'add_character',name:'Rollback',position:{x:0,y:0},color:[255,255,255,255]},{type:'remove',id:randomUUID()}]}});assert.equal(invalid.isError,true);
  assert.equal((await tool('inspect_project',{project})).document.characters.length,2);
  const preview=await rpc('tools/call',{name:'render_preview',arguments:{project,frame:104,width:960,height:540,path:'preview.png'}});assert.ok(preview.content.some(c=>c.type==='image'));
  await tool('save_project',{project,path:'agent-fight.astral'});
  const media=await tool('export_media',{project,spec:{path:'agent-social.png',preset:'instagram_reels',width:180,height:320,start:104,end:104}});
  let mediaState;
  for(let i=0;i<400;i++){mediaState=await tool('job_status',{job:media.job});if(['completed','failed','cancelled'].includes(mediaState.state))break;await new Promise(resolve=>setTimeout(resolve,25));}
  assert.equal(mediaState.state,'completed',JSON.stringify(mediaState));assert.equal(mediaState.plan.width,180);assert.equal(mediaState.plan.height,320);
  // Cancellation must preserve a previous complete destination.
  await writeFile(path.join(workspace,'cancelled.mp4'),'previous complete output');
  const cancellation=await tool('export_video',{project,spec:{path:'cancelled.mp4',hardware:false}});
  await tool('cancel_job',{job:cancellation.job});
  let cancelled;
  for(let i=0;i<200;i++){
    cancelled=await tool('job_status',{job:cancellation.job});
    if(['cancelled','failed','completed'].includes(cancelled.state))break;
    await new Promise(resolve=>setTimeout(resolve,25));
  }
  assert.equal(cancelled.state,'cancelled');
  assert.equal(await readFile(path.join(workspace,'cancelled.mp4'),'utf8'),'previous complete output');
  // Exercise the transitional decoder without claiming native legacy parity.
  const legacyRaw=Buffer.from(JSON.stringify(createStartupDocument()));
  const legacyPacked=deflateSync(legacyRaw);const header=Buffer.alloc(56);
  header.write('STYK');header[4]=1;header[5]=1;header.writeBigUInt64LE(BigInt(legacyRaw.length),8);header.writeBigUInt64LE(BigInt(legacyPacked.length),16);createHash('sha256').update(legacyRaw).digest().copy(header,24);
  const legacyFile=Buffer.concat([header,legacyPacked]);await writeFile(path.join(workspace,'legacy.styk'),legacyFile);
  const legacy=await tool('import_legacy',{path:'legacy.styk'});assert.ok(legacy.warnings.some(w=>w.includes('sampled at project FPS')));
  const archived=(await tool('inspect_project',{project:legacy.project})).document;
  assert.equal(archived.metadata.legacy.conversion,'source-evaluated-native-cels');
  assert.equal(archived.metadata.legacy.original_bytes,legacyFile.length);
  assert.ok(!('original_source' in archived));
  await tool('save_project',{project:legacy.project,path:'archived-legacy.astral'});
  const savedArchive=JSON.parse(inflateSync((await readFile(path.join(workspace,'archived-legacy.astral'))).subarray(60)));
  assert.deepEqual(Buffer.from(savedArchive.original_source.source_base64,'base64'),legacyFile);
  assert.deepEqual(JSON.parse(savedArchive.original_source.canonical_json).figure_types,JSON.parse(legacyRaw).figure_types);
  const file=await readFile(path.join(workspace,'preview.png'));assert.equal(file.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
  const features=await featureAcceptance(tool,workspace,executable);
  const cels=await celAcceptance(tool,workspace,executable);
  const report={tools:tools.tools.length,characters:after.characters.length,actions:after.actions.length,effects:after.effects.length,audio:after.audio.length,revision,preview_sha256:createHash('sha256').update(file).digest('hex'),stale_revision_rejected:true,atomic_rollback:true,targeted_revision_preserved_tracks:true,cancel_preserved_previous_output:true,legacy_source_preserved:true,features};
  report.cels=cels;
  report.assistance=await assistanceAcceptance(tool,workspace);
  report.motion=await motionAcceptance(tool,rpc);
  await writeFile(path.join(workspace,'mcp-report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));
}finally{child.stdin.end();setTimeout(()=>child.kill(),1000).unref();}
