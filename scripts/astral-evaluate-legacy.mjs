// Transitional, read-only evaluator. Native output is editable and never needs this helper for playback.
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
import {timeline,evaluatedFrame,buildGeometry} from '../src/pivot/geometry.mjs';
import {effectiveCamera,sceneBackgroundColor,inverseCameraScale,cameraIsIdentity} from '../src/pivot/background.mjs';

export function evaluateLegacy(report,{maxFrames=12000,maxPoints=2000000}={}){
  const rows=timeline(report),duration=Math.max(1,...rows.map(row=>row.start+row.count));
  if(duration>maxFrames)throw new Error(`Evaluated legacy import exceeds ${maxFrames} frames; original is retained`);
  const samples=[],warnings=new Set(),images={};let points=0,index=0;
  for(let tick=0;tick<duration;tick++){
    while(index+1<rows.length&&rows[index+1].start<=tick)index++;
    const row=rows[index];if(!row)break;
    const frame=evaluatedFrame(report,row.index,row.count?(tick-row.start)/row.count:0,tick),geometry=buildGeometry(report,frame);
    for(const warning of geometry.warnings)warnings.add(warning);
    const sourceCamera=effectiveCamera(report,frame),camera=cameraIsIdentity(sourceCamera)?null:sourceCamera,center={x:report.canvas_width/2+Number(camera?.x??0),y:report.canvas_height/2+Number(camera?.y??0)},zoom=inverseCameraScale(camera),angle=-Number(camera?.angle_rad??0);
    const rotate=p=>({x:center.x+(p.x-center.x)*Math.cos(angle)-(p.y-center.y)*Math.sin(angle),y:center.y+(p.x-center.x)*Math.sin(angle)+(p.y-center.y)*Math.cos(angle)});
    if(angle)warnings.add('Camera rotation is baked into sampled native points; translation and zoom remain camera keys.');
    const rigs=[];
    for(const g of geometry.orderedObjects){
      // Movieclip-expanded items can have a different source object; only bind stable top-level geometry.
      const joints=[...g.joints].map(([id,p])=>({id,position:rotate(p),parent:g.fig.segments.find(s=>s.segment_id===id)?.parent_id??null}));
      rigs.push({uid:g.obj.unique_id,type:g.fig.figure_type_id,name:g.fig.name??'Imported rig',root:rotate(g.root),joints});
    }
    const strokes=[];
    for(const d of geometry.drawables){
      if(d.culled)continue;
      const rgb=d.rgb??[0,0,0],color=[...rgb,Math.round(Math.max(0,Math.min(1,(d.alpha??1)*(d.opacity??1)))*255)];
      if(!color[3])continue;
      if(d.seg?.paint||d.poly?.paint)warnings.add('Gradient/pattern paint currently uses its source base colour.');
      if(d.obj?._stykVfx||d.seg?._stykVfx)warnings.add('Source VFX stacks remain archived; their filter pixels are not baked into vector cels.');
      const common={color,width:Math.max(0.1,Math.min(500,d.thickness??1)),uid:d.obj?.unique_id,from:d.seg?.parent_id,to:d.seg?.segment_id};
      if(d.type==='poly')strokes.push({...common,points:d.points.map(rotate),filled:!d.outline});
      else if(d.type==='line'&&d.thickness>0)strokes.push({...common,points:d.points.map(rotate),filled:false});
      else if(d.type==='circle'){
        const filled=d.seg.segment_kind==='circleFill'||d.thickness>=Math.abs(d.length),radius=(Math.abs(d.length)+(filled?d.thickness:0))/2,mid={x:(d.start.x+d.end.x)/2,y:(d.start.y+d.end.y)/2};
        const circle=Array.from({length:65},(_,i)=>rotate({x:mid.x+radius*Math.cos(i*Math.PI/32),y:mid.y+radius*Math.sin(i*Math.PI/32)}));
        if(!filled&&d.seg.segment_kind==='circle')strokes.push({...common,points:circle,color:[255,255,255,color[3]],filled:true});
        strokes.push({...common,points:circle,filled});
      }else if(d.type==='image'){
        const a=d.asset,url=a?.data_url??a?.data_uri??a?.url;
        if(!a?.anchor_length||!url?.startsWith('data:image/')||d.fig?._styk?.drawing?.version===2){warnings.add('Unresolved image/tiled-drawing source remains archived.');continue;}
        const hash=createHash('sha256').update(url).digest('hex');images[hash]=url;
        const reverse=Boolean(d.seg.extra_u8_a??d.seg.extra?.[1]),mirror=Boolean(d.seg.extra_u8_b??d.seg.extra?.[2])!==Boolean(d.obj.image_text_flags),start=reverse?d.end:d.start;
        const angle=(mirror?d.angle+a.anchor_angle_rad-Math.PI:d.angle-a.anchor_angle_rad)+(reverse?Math.PI:0),scale=Math.hypot(d.end.x-d.start.x,d.end.y-d.start.y)/a.anchor_length;
        const width=a.display_width??a.width,height=a.display_height??a.height,anchor=a.anchor_start;
        if(!anchor||!width||!height){warnings.add('Image dimensions/anchors are missing; original retained.');continue;}
        const map=(x,y)=>{x=(x-anchor.x)*scale*(mirror?-1:1);y=(y-anchor.y)*scale;return rotate({x:start.x+x*Math.cos(angle)-y*Math.sin(angle),y:start.y+x*Math.sin(angle)+y*Math.cos(angle)});};
        strokes.push({...common,bitmap:hash,points:[[0,0],[width,0],[width,height],[0,height]].map(([x,y])=>map(x,y)),filled:false});
      }else if(d.type==='text')warnings.add('Source text outlines remain archived; use native editable dialogue to replace them.');
      else warnings.add(`Unsupported source drawable: ${d.type}`);
    }
    points+=strokes.reduce((n,s)=>n+s.points.length,0)+rigs.reduce((n,r)=>n+r.joints.length,0);
    if(points>maxPoints)throw new Error('Evaluated import exceeds the two-million-point budget; original is retained');
    const raw=sceneBackgroundColor(report,frame)??report.backgrounds?.find(b=>b.background_id===frame.background_id)?.color;
    const background=raw===undefined?[255,255,255,255]:[(raw>>>16)&255,(raw>>>8)&255,raw&255,(raw>>>24)&255];
    const backdrop=report.backgrounds?.find(b=>b.background_id===frame.background_id);
    if(backdrop&&raw===undefined)warnings.add('Source image/gradient backgrounds remain archived; native stage uses a plain fallback.');
    if(samples.length&&JSON.stringify(background)!==JSON.stringify(samples[0].background))warnings.add('Animated background colours remain archived; native stage uses the initial colour.');
    samples.push({frame:tick,rigs,strokes,camera:{center,zoom},background});
  }
  return {duration,samples,images,warnings:[...warnings],method:'source-evaluated-native-cels-v1'};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
  const chunks=[];let size=0;for await(const chunk of process.stdin){size+=chunk.length;if(size>128*1024*1024)throw new Error('Source document exceeds 128 MiB');chunks.push(chunk);}const bytes=Buffer.concat(chunks);
  const result=JSON.stringify(evaluateLegacy(JSON.parse(bytes)));if(Buffer.byteLength(result)>128*1024*1024)throw new Error('Evaluated output exceeds 128 MiB');process.stdout.write(result);
}
