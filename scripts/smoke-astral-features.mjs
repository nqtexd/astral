import assert from 'node:assert/strict';
import {randomUUID,createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import path from 'node:path';

// The caller supplies a real stdio MCP client; this exercises the public tools.
export async function featureAcceptance(tool,workspace,executable){
  const created=await tool('create_project',{name:'Learning and motion graphics',width:960,height:540,fps:30,duration:120});
  const project=created.project;let revision=created.revision;
  const apply=async commands=>{const result=await tool('apply_commands',{project,expected_revision:revision,commands});revision=result.revision;return result;};
  const added=await apply([{type:'add_character',name:'Luma',position:{x:240,y:430},color:[188,166,255,255]}]);const character=added.created_ids[0];
  const rig=(await tool('inspect_project',{project})).document.characters[0];
  const middle=structuredClone(rig.keys[0].pose);middle.root={x:300,y:300};middle.joints[6].y-=30;
  const last=structuredClone(rig.keys[0].pose);last.root={x:360,y:430};
  await apply([{type:'set_pose',character,frame:15,pose:middle,easing:'linear'},{type:'set_pose',character,frame:30,pose:last,easing:'linear'}]);
  const learned=await tool('learn_motion',{project,expected_revision:revision,character,smoothing:0.0001});revision=learned.revision;
  const sourceIds=[];const drawings=[];
  for(const [frame,y]of[[0,410],[15,340],[30,410]]){const id=randomUUID();sourceIds.push(id);drawings.push({type:'add_drawing',drawing:{id,name:'Ribbon',start:frame,end:119,points:[{x:30,y},{x:95,y:y-20},{x:180,y}],color:[90,215,195,255],width:4}});}
  await apply(drawings);
  const stroke=await tool('learn_drawing',{project,expected_revision:revision,sources:sourceIds,smoothing:0.0001});revision=stroke.revision;
  for(const graphic of [{kind:'cloud',position:{x:180,y:80},end_position:{x:600,y:80},size:{x:240,y:110},color:[220,235,255,210]},{kind:'star',position:{x:790,y:90},end_position:{x:790,y:150},size:{x:100,y:100},color:[255,200,125,255]},{kind:'lower_third',position:{x:480,y:480},end_position:{x:480,y:480},size:{x:600,y:80},color:[100,65,170,255],text:'CREATED THROUGH MCP'}]){const result=await tool('add_motion_graphic',{project,expected_revision:revision,start:0,end:119,...graphic});revision=result.revision;}
  const dialogue=await tool('add_dialogue',{project,expected_revision:revision,text:'Three example drawings become editable inbetweens. The dialogue stays editable too.',speaker:'ASTRAL',start:0,end:119,position:{x:380,y:210},style:'caption',typewriter:30});revision=dialogue.revision;
  const document=(await tool('inspect_project',{project,frame:7}));
  assert.equal(document.document.learned_motions.length,1);assert.equal(document.document.drawing_motions.length,1);assert.equal(document.document.graphics.length,3);assert.equal(document.document.dialogues.length,1);assert.equal(document.document.characters[0].keys.length,3);
  assert.ok(document.scene.characters[0].pose.root.y<420);
  await tool('render_preview',{project,frame:22,width:960,height:540,path:'features-before-bake.png'});
  const before=await readFile(path.join(workspace,'features-before-bake.png'));
  await tool('save_project',{project,path:'features-learned.astral'});
  execFileSync(executable,['--workspace',workspace,'render','features-learned.astral','features-reopened.png','--frame','22'],{windowsHide:true});
  assert.deepEqual(before,await readFile(path.join(workspace,'features-reopened.png')));
  await apply([{type:'bake_motion',motion:learned.created_ids[0]},{type:'bake_drawing',motion:stroke.created_ids[0]}]);
  await tool('render_preview',{project,frame:22,width:960,height:540,path:'features-after-bake.png'});
  assert.deepEqual(before,await readFile(path.join(workspace,'features-after-bake.png')));
  const baked=(await tool('inspect_project',{project})).document;
  assert.equal(baked.characters[0].keys.length,31);assert.equal(baked.drawings.length,31);assert.equal(baked.learned_motions.length,0);assert.equal(baked.drawing_motions.length,0);
  await tool('save_project',{project,path:'features-baked.astral'});
  return{pose_examples:3,drawing_examples:3,baked_pose_frames:31,baked_drawing_frames:31,graphics:3,dialogues:1,preview_sha256:createHash('sha256').update(before).digest('hex'),save_reopen_identical:true,baked_export_identical:true};
}
