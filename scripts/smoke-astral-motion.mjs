import assert from 'node:assert/strict';

// Actual MCP calls, including non-mutating proposals and one-transaction commits.
export async function motionAcceptance(tool,rpc){
  const p=await tool('create_project',{name:'Motion studio acceptance',width:960,height:540,fps:24,duration:240});
  const project=p.project;let revision=p.revision;
  const inspect=async()=>(await tool('inspect_project',{project})).document;
  async function plan(task,frame=0){
    const before=await inspect();
    const result=await tool('motion_tools',{operation:'edit',project,expected_revision:revision,tool:task,apply:false,preview_frame:frame});
    assert.deepEqual(await inspect(),before,'A proposal changed the document');
    return result.plan;
  }
  async function commit(task,frame=0){
    const proposal=await plan(task,frame);
    if(proposal.commands.length){const result=await tool('apply_commands',{project,expected_revision:revision,commands:proposal.commands});revision=result.revision;}
    return proposal;
  }
  for(const [name,x,color] of [['Nova',350,[45,60,90,255]],['Orion',600,[80,125,180,255]]])
    await commit({task:'rig',template:'human',name,position:{x,y:340},height:220,color});
  let doc=await inspect();const [first,second]=doc.characters.map(c=>c.id);
  const dry={operation:'edit',project,expected_revision:revision,tool:{task:'smart_pose',character:first,frame:0,joint:'right_hand',target:{x:410,y:300},style:'natural'},apply:false,preview_frame:0};
  const comparison=await rpc('tools/call',{name:'motion_tools',arguments:dry});
  assert.ok(!comparison.isError,JSON.stringify(comparison));assert.equal(comparison.content.filter(c=>c.type==='image').length,2);assert.ok(!('image_base64' in comparison.structuredContent));assert.ok(!('original_image_base64' in comparison.structuredContent));
  const stale=await rpc('tools/call',{name:'motion_tools',arguments:{...dry,expected_revision:revision-1,apply:true}});assert.equal(stale.isError,true);
  const invalidPreview=await rpc('tools/call',{name:'motion_tools',arguments:{...dry,apply:true,preview_frame:999999}});assert.equal(invalidPreview.isError,true);assert.deepEqual(await inspect(),doc);

  const fight=await commit({task:'fight',first,second,start:0,beats:['punch','dodge','counter','kick','knockback'].map((action,i)=>({attacker:i%2?second:first,action,frames:48,strength:1})),style:'anime_fight',impacts:true,overwrite:true},12);
  assert.equal(Object.keys(fight.markers).length,5);doc=await inspect();assert.ok(doc.characters.every(c=>c.keys.length>10));assert.ok(doc.effects.length>=10);
  await commit({task:'camera',start:0,end:239,character:first,from:{x:0,y:-40},to:{x:60,y:-20},zoom_start:1,zoom_end:1.2});
  const wave=await tool('import_asset',{project,expected_revision:revision,path:'assets/impact.wav',tags:['impact']});revision=wave.revision;
  await commit({task:'impact',character:first,joint:'right_hand',frame:12,action:null,pack:'heavy',sound:wave.created_ids[0],sound_seconds:0.25},12);
  const waveform=await tool('motion_tools',{operation:'waveform',project,asset:wave.created_ids[0],columns:128});assert.equal(waveform.peaks.length,128);
  await commit({task:'save_library',name:'Five-beat combo',kind:'combo',characters:[first,second],layer:null,start:0,end:239});
  await tool('motion_tools',{operation:'export_library',project,path:'library/fight.astral-library.json',entries:[]});
  const imported=await tool('motion_tools',{operation:'import_library',project,expected_revision:revision,path:'library/fight.astral-library.json'});revision=imported.revision;assert.equal(imported.imported,1);
  const before=await inspect();
  await commit({task:'motion_brush',character:first,joint:'root',path:[{x:350,y:340},{x:420,y:300},{x:500,y:340}],start:144,end:167,keys:24,easing:'smooth',style:'natural'},155);
  const after=await inspect();assert.deepEqual(after.characters[1],before.characters[1]);assert.deepEqual(after.effects,before.effects);assert.deepEqual(after.audio,before.audio);assert.deepEqual(after.camera,before.camera);
  const review=await plan({task:'analyze',start:0,end:239,characters:[first,second],tolerance:0.12},null);assert.ok(Array.isArray(review.diagnostics));
  const path=await plan({task:'trajectory',character:first,joint:'right_hand',start:0,end:239,step:2},null);assert.ok(path.path.length>30);
  const undo=await tool('history',{project,expected_revision:revision,redo:false});revision=undo.revision;assert.deepEqual((await inspect()).characters,before.characters);
  const redo=await tool('history',{project,expected_revision:revision,redo:true});revision=redo.revision;assert.deepEqual((await inspect()).characters,after.characters);
  await tool('save_project',{project,path:'motion-studio.astral'});
  return {editable_fight_beats:5,scene_seconds:10,comparison_images:2,stale_and_invalid_preview_rejected:true,pose_keys:after.characters.reduce((n,c)=>n+c.keys.length,0),effect_tracks:after.effects.length,audio_tracks:after.audio.length,non_mutating_previews:true,targeted_edit_preserves_other_tracks:true,library_file_roundtrip:true,waveform_columns:128,undo_redo:true};
}
