import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import path from 'node:path';

export async function assistanceAcceptance(tool,workspace){
  const created=await tool('create_managed_project',{name:'Assistance acceptance',width:256,height:320,fps:24,duration:48,path:'Assistance/Film.astral'});
  const project=created.project;let revision=created.revision;
  const inspect=async()=> (await tool('inspect_project',{project})).document;
  const figure=await tool('create_character',{project,expected_revision:revision,name:'Reference',position:{x:128,y:210}});revision=figure.revision;
  const character=figure.created_ids[0];
  await tool('render_preview',{project,frame:0,width:256,height:320,path:'reference.png'});
  const mapped=await tool('recognize_stickman',{project,path:'reference.png'});
  assert.equal(mapped.detection.landmarks.length,11);assert.equal(mapped.review_required,true);
  const imported=await tool('import_stickman_rig',{project,expected_revision:revision,path:'reference.png',detection:mapped.detection,name:'Mapped',center:{x:128,y:210},height:230,frame:0});revision=imported.revision;
  const animated=await tool('animate_character',{project,expected_revision:revision,character:imported.character,action:'walk',start:0,end:47,distance:60});revision=animated.revision;
  const proposal=await tool('complete_pose',{project,expected_revision:revision,character,frame:5,observed:['hips','chest','head']});
  assert.equal(proposal.observed_preserved,true);assert.equal((await inspect()).revision,revision);
  assert.equal(proposal.frame,5);
  const layer=(await inspect()).layers[0].id;
  const strokes=y=>[0,30].map(offset=>({points:[{x:20,y:y+offset},{x:60,y:y+5+offset},{x:100,y:y+offset}],width:3,color:[10,10,10,255]}));
  for(const[frame,y]of [[0,100],[10,50],[20,100]]){
    revision=(await tool('set_drawing_frame',{project,expected_revision:revision,layer,frame,strokes:strokes(y)})).revision;
  }
  revision=(await tool('set_drawing_frame',{project,expected_revision:revision,layer,frame:5,strokes:[strokes(123)[0]]})).revision;
  const completion=await tool('complete_frame',{project,expected_revision:revision,layer,sources:[0,10,20],frame:5});
  assert.equal((await inspect()).revision,revision);assert.equal(completion.commands[0].strokes.length,2);
  assert.deepEqual(completion.commands[0].strokes[0],strokes(123)[0]);
  async function wait(value){for(let i=0;i<2400;i++){const v=await tool('job_status',{job:value.job});if(['completed','failed','cancelled'].includes(v.state)){assert.equal(v.state,'completed',JSON.stringify(v));return v;}await new Promise(r=>setTimeout(r,25));}throw Error('Job timeout');}
  const generation=await wait(await tool('generate_inbetweens',{project,expected_revision:revision,layer,sources:[0,10,20],start:0,end:20}));revision=generation.revision;
  assert.equal(generation.generated_frames,17);
  const trained=await wait(await tool('train_pose_model',{project,expected_revision:revision,samples:512,epochs:2}));revision=trained.revision;
  assert.equal(trained.report.training_samples,512);assert.equal(trained.report.parameters,3606);
  const model=JSON.parse(await readFile(path.join(workspace,'Assistance',trained.path),'utf8'));assert.equal(model.version,1);
  await tool('complete_pose',{project,expected_revision:revision,character,frame:5,observed:['hips','chest'],model_path:trained.path});
  const cancelled=await tool('train_pose_model',{project,expected_revision:revision,samples:32768,epochs:300});
  await tool('cancel_job',{job:cancelled.job});
  let cancelledState;
  for(let i=0;i<2400;i++){cancelledState=await tool('job_status',{job:cancelled.job});if(['completed','failed','cancelled'].includes(cancelledState.state))break;await new Promise(r=>setTimeout(r,25));}
  assert.equal(cancelledState.state,'cancelled');assert.equal((await inspect()).revision,revision);
  const staleTraining=await tool('train_pose_model',{project,expected_revision:revision,samples:32768,epochs:1});
  revision=(await tool('apply_commands',{project,expected_revision:revision,commands:[{type:'set_project',name:'Concurrent human edit',duration:null,background:null}]})).revision;
  let staleState;
  for(let i=0;i<2400;i++){staleState=await tool('job_status',{job:staleTraining.job});if(['completed','failed','cancelled'].includes(staleState.state))break;await new Promise(r=>setTimeout(r,25));}
  assert.equal(staleState.state,'failed');assert.equal((await inspect()).revision,revision);assert.equal((await inspect()).assets.filter(a=>a.kind==='model').length,1);
  const voices=await tool('list_voices');assert.ok(voices.voices.length>0);
  const voiced=await wait(await tool('add_voice',{project,expected_revision:revision,text:'Astral creates editable animation, with offline speech and dialogue.',start:0,character:imported.character,dialogue:true,formatting:{font:'bold',align:'center',line_height:1.5,padding:16}}));revision=voiced.revision;
  const document=await inspect();assert.equal(document.audio.length,1);assert.equal(document.dialogues.length,1);assert.equal(document.schema,4);
  await tool('save_project',{project,path:'Film.astral'});
  const preview=await tool('render_preview',{project,frame:10,width:512,height:640,path:'finished.png'});assert.ok(preview);
  return {managed_project:true,mapped_landmarks:11,editable_actions:true,nonmutating_pose_proposal:true,generated_frames:17,local_training:true,training_cancellation:true,training_stale_revision_rollback:true,offline_voice:true,rich_dialogue:true,revision};
}
