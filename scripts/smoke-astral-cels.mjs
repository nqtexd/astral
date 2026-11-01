import assert from 'node:assert/strict';
import {randomUUID,createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {spawnSync} from 'node:child_process';
import path from 'node:path';

export async function celAcceptance(tool,workspace,executable){
  const created=await tool('create_project',{name:'Frame study',width:640,height:360,fps:24,duration:24});
  const project=created.project;let revision=created.revision;
  const layer=randomUUID();
  async function apply(commands){const value=await tool('apply_commands',{project,expected_revision:revision,commands});revision=value.revision;return value;}
  const stroke=(dy,color=[30,30,30,255])=>({points:[{x:100,y:200+dy},{x:250,y:120+dy},{x:450,y:190+dy}],width:8,color});
  await apply([{type:'set_project',name:null,duration:null,background:[250,250,250,255]},{type:'add_layer',layer:{id:layer,name:'Ink',visible:true,locked:false,opacity:1}},{type:'set_cel',layer,frame:0,strokes:[stroke(0)]},{type:'set_cel',layer,frame:6,strokes:[stroke(-40)]},{type:'set_cel',layer,frame:12,strokes:[]}]);
  const before=(await tool('inspect_project',{project})).document;
  assert.equal(before.schema,3);
  await apply([{type:'set_cel',layer,frame:4,strokes:[stroke(10),stroke(30,[60,120,180,255])]}]);
  const after=(await tool('inspect_project',{project})).document;
  assert.deepEqual(after.cels.find(c=>c.frame===0),before.cels.find(c=>c.frame===0));
  assert.deepEqual(after.cels.find(c=>c.frame===6),before.cels.find(c=>c.frame===6));
  await apply([{type:'move_cel',layer,from:6,to:8}]);
  const keyed=(await tool('inspect_project',{project})).document;
  assert.equal(keyed.cels.find(c=>c.frame===8).id,before.cels.find(c=>c.frame===6).id);
  await apply([{type:'set_exposure',layer,frame:8,frames:3}]);
  const held=(await tool('inspect_project',{project})).document;
  assert.ok(held.cels.some(c=>c.frame===11&&c.strokes.length===0));
  await tool('render_preview',{project,frame:8,width:640,height:360,path:'cel-preview.png'});
  await tool('save_project',{project,path:'frame-study.astral'});
  const render=spawnSync(executable,['--workspace',workspace,'render','frame-study.astral','cel-reopened.png','--frame','8','--width','640','--height','360'],{encoding:'utf8',windowsHide:true});
  assert.equal(render.status,0,render.stderr);
  const preview=await readFile(path.join(workspace,'cel-preview.png'));
  assert.deepEqual(await readFile(path.join(workspace,'cel-reopened.png')),preview);
  const renderJob=await tool('export_media',{project,spec:{path:'cel-preview.mp4',width:320,height:180,start:0,end:11,fps:24}});
  let state;
  for(let i=0;i<600;i++){state=await tool('job_status',{job:renderJob.job});if(['completed','failed','cancelled'].includes(state.state))break;await new Promise(r=>setTimeout(r,25));}
  assert.equal(state.state,'completed',JSON.stringify(state));assert.equal(state.plan.frames,12);assert.equal(state.plan.fps,24);
  return {layers:held.layers.length,cels:held.cels.length,independent_edits:true,stable_key_identity:true,exposure_preserved:true,save_reopen_identical:true,video_frames:state.plan.frames,preview_sha256:createHash('sha256').update(preview).digest('hex')};
}
