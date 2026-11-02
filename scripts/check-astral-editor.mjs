// Public stdio protocol acceptance for editor channels and portable media.
import {client} from '../tests/helpers/astral-client.mjs';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import path from 'node:path';
import {randomUUID,createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const root=path.resolve(import.meta.dirname,'..'),workspace=path.join(root,'artifacts','astral-editor-015');await mkdir(workspace,{recursive:true});
const binary=process.env.ASTRAL_BINARY??path.join(root,'target/debug/astral.exe');const c=await client(binary,workspace);const checks={};
try{
 const project=(await c.tool('create_project',{name:'Editor acceptance',width:320,height:180,fps:24,duration:96})).project;
 let revision=0;
 const actor=await c.tool('create_character',{project,expected_revision:revision,name:'Actor',position:{x:100,y:140}});revision=actor.revision;const character=actor.created_ids[0];
 const track={id:randomUUID(),name:'Root X',character,joint:null,axis:'x',keys:[{frame:0,value:40,incoming:2,outgoing:2,interpolation:'cubic'},{frame:24,value:88,incoming:2,outgoing:2,interpolation:'cubic'}]};
 revision=(await c.tool('apply_commands',{project,expected_revision:revision,commands:[{type:'set_curve_tracks',tracks:[track]}]})).revision;
 assert.equal((await c.tool('inspect_project',{project,frame:12})).scene.characters[0].pose.root.x,64);checks.hermite_channel_evaluation=true;
 const duplicate=await c.rpc('tools/call',{name:'apply_commands',arguments:{project,expected_revision:revision,commands:[{type:'set_curve_tracks',tracks:[track,{...track,id:randomUUID()}]}]}});assert.equal(duplicate.isError,true);assert.equal((await c.tool('inspect_project',{project})).document.revision,revision);checks.duplicate_channel_rollback=true;
 await writeFile(path.join(workspace,'source.txt'),'portable media');await writeFile(path.join(workspace,'replacement.txt'),'relinked media');
 const asset=randomUUID();revision=(await c.tool('apply_commands',{project,expected_revision:revision,commands:[{type:'add_asset',asset:{id:asset,name:'Source',kind:'source',path:'source.txt',tags:[]}}]})).revision;
 const inspect=await c.tool('asset_tools',{action:'inspect',project});assert.equal(inspect.assets[0].available,true);
 revision=(await c.tool('asset_tools',{action:'relink',project,expected_revision:revision,asset,path:'replacement.txt'})).revision;
 assert.equal((await c.tool('inspect_project',{project})).document.assets[0].id,asset);checks.stable_identity_relink=true;
 const escape=await c.rpc('tools/call',{name:'asset_tools',arguments:{action:'relink',project,expected_revision:revision,asset,path:'../outside.txt'}});assert.equal(escape.isError,true);checks.relink_confinement=true;
 const archive=`exports/${randomUUID()}.astralpack`,destination=`unpacked/${randomUUID()}`;
 await c.job(await c.tool('asset_tools',{action:'package',project,expected_revision:revision,path:archive}));
 const unpack=await c.job(await c.tool('asset_tools',{action:'unpack',path:archive,destination}));assert.equal(await readFile(path.join(workspace,destination,'replacement.txt'),'utf8'),'relinked media');
 await c.tool('close_project',{project,expected_revision:revision,discard:true});
 const reopened=await c.tool('open_project',{path:unpack.path});assert.equal(reopened.project,project);assert.equal((await c.tool('inspect_project',{project,frame:12})).scene.characters[0].pose.root.x,64);checks.packaged_open_editable_channels=true;
 const corrupt=await readFile(path.join(workspace,archive));corrupt[corrupt.length>>1]^=0xff;const bad=`bad-${randomUUID()}.astralpack`;await writeFile(path.join(workspace,bad),corrupt);
 const job=await c.tool('asset_tools',{action:'unpack',path:bad,destination:`bad-output/${randomUUID()}`});let status;for(let i=0;i<200;i++){status=await c.tool('job_status',{job:job.job});if(['completed','failed','cancelled'].includes(status.state))break;await new Promise(r=>setTimeout(r,25));}assert.equal(status.state,'failed');checks.corrupt_package_rejected=true;
 const report={version:'0.15.0',binary_sha256:createHash('sha256').update(await readFile(binary)).digest('hex'),passed:true,checks};await writeFile(path.join(workspace,'report.json'),JSON.stringify(report,null,2));console.log(report);
}finally{c.close();}
