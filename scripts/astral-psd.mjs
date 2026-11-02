// Raw image-data decoding avoids browser canvas and its alpha premultiplication.
import ag from '../vendor/psd/ag-psd-34.1.0.cjs';
import {readSource} from './astral-source-input.mjs';
import {deflateSync} from 'node:zlib';
import {pathToFileURL} from 'node:url';
ag.initializeCanvas(()=>{throw Error('Canvas-only PSD feature is unsupported');},(width,height)=>({width,height,data:new Uint8ClampedArray(width*height*4)}));
function crc(bytes){let c=0xffffffff;for(const b of bytes){c^=b;for(let i=0;i<8;i++)c=(c>>>1)^((c&1)?0xedb88320:0);}return(c^0xffffffff)>>>0;}
function chunk(type,data){const name=Buffer.from(type),out=Buffer.alloc(data.length+12);out.writeUInt32BE(data.length);name.copy(out,4);data.copy(out,8);out.writeUInt32BE(crc(Buffer.concat([name,data])),data.length+8);return out;}
function png(image){const{width:w,height:h,data}=image;if(w*h>16777216||w<1||h<1||data.length!==w*h*4)throw Error('Invalid PSD image bounds');const rows=Buffer.alloc(h*(w*4+1));for(let y=0;y<h;y++)Buffer.from(data.buffer,data.byteOffset+y*w*4,w*4).copy(rows,y*(w*4+1)+1);const header=Buffer.alloc(13);header.writeUInt32BE(w);header.writeUInt32BE(h,4);header[8]=8;header[9]=6;return Buffer.concat([Buffer.from('89504e470d0a1a0a','hex'),chunk('IHDR',header),chunk('IDAT',deflateSync(rows)),chunk('IEND',Buffer.alloc(0))]).toString('base64');}
// Layer rectangles are independent of the document rectangle. Reject hostile
// off-canvas allocations before the third-party decoder creates TypedArrays.
export function preflight(bytes){
 let offset=26;const need=(at,n)=>{if(at<0||n<0||at+n>bytes.length)throw Error('Truncated PSD section');};
 const section=()=>{need(offset,4);const length=bytes.readUInt32BE(offset);offset+=4;need(offset,length);const at=offset;offset+=length;return{at,length};};
 section();section();const layerMask=section();if(!layerMask.length)return;
 need(layerMask.at,4);const length=bytes.readUInt32BE(layerMask.at);if(!length)return;const end=layerMask.at+4+length;if(end>layerMask.at+layerMask.length)throw Error('Invalid PSD layer section');
 offset=layerMask.at+4;need(offset,2);const count=Math.abs(bytes.readInt16BE(offset));offset+=2;if(count>64)throw Error('PSD layer count exceeds budget');let pixels=0,channels=0;
 const bounds=at=>{need(at,16);const h=bytes.readInt32BE(at+8)-bytes.readInt32BE(at),w=bytes.readInt32BE(at+12)-bytes.readInt32BE(at+4);if(w<0||h<0||w>8192||h>8192)throw Error('PSD layer/mask dimensions exceed budget');pixels+=w*h;if(pixels>33554432)throw Error('PSD layer pixels exceed budget');return w*h;};
 let encoded=0;
 for(let i=0;i<count;i++){
  const area=bounds(offset);offset+=16;need(offset,2);const n=bytes.readUInt16BE(offset);offset+=2;if(n>64)throw Error('PSD channel count exceeds budget');channels+=area*n;if(channels>134217728)throw Error('PSD decoded channels exceed budget');need(offset,n*6+16);
  for(let j=0;j<n;j++){encoded+=bytes.readUInt32BE(offset+2);offset+=6;}
  offset+=12;need(offset,4);const extra=bytes.readUInt32BE(offset);offset+=4;need(offset,extra);if(extra>=4){const mask=bytes.readUInt32BE(offset);if(mask&&mask>=20){if(mask>extra-4)throw Error('Invalid PSD mask bounds');bounds(offset+4);}}
  offset+=extra;if(offset>end)throw Error('PSD layer records exceed section');
 }
 if(offset+encoded>end)throw Error('PSD channel data exceeds section');
}
export function decodePsd(bytes){
 if(bytes.length>33554432||bytes.length<26||bytes.readUInt16BE(22)!==8||bytes.readUInt16BE(24)!==3)throw Error('PSD import requires an 8-bit RGB document up to 32 MiB');
 const w=bytes.readUInt32BE(18),h=bytes.readUInt32BE(14);if(w<1||h<1||w>8192||h>8192||w*h>16777216)throw Error('PSD canvas exceeds budget');
 preflight(bytes);
 let doc=ag.readPsd(bytes,{useImageData:true,skipCompositeImageData:true,skipThumbnail:true,throwForMissingFeatures:false});
 let count=0,pixels=0;const warnings=new Set();
 function walk(l,depth){if(++count>64||depth>12)throw Error('PSD exceeds 64 layers or 12 group levels');const name=String(l.name??'PSD layer').slice(0,512);const result={name,visible:!l.hidden,opacity:l.opacity??1,blend:l.blendMode??'normal',left:l.left??0,top:l.top??0,width:Math.max(1,(l.right??w)-(l.left??0)),height:Math.max(1,(l.bottom??h)-(l.top??0))};
  if(l.children){result.children=l.children.map(c=>walk(c,depth+1));if(result.blend==='pass through'){warnings.add('Pass-through groups are isolated native groups; cross-group adjustments may differ.');result.blend='normal';}}
  else if(l.imageData){pixels+=l.imageData.width*l.imageData.height;if(pixels>33554432)throw Error('PSD decoded pixels exceed 32 megapixels');result.png=png(l.imageData);result.width=l.imageData.width;result.height=l.imageData.height;}
  if(l.text){result.text={value:l.text.text,size:l.text.style?.fontSize??24,color:l.text.style?.fillColor??{r:0,g:0,b:0},font:l.text.style?.font?.name??'',align:l.text.paragraphStyle?.justification??'left',transform:l.text.transform??[1,0,0,1,0,0]};warnings.add('Text is editable with Astral fonts; mixed text runs, arbitrary affine transforms and Photoshop font metrics are not equivalent.');}
  if(l.mask&&!l.mask.disabled&&l.mask.imageData){pixels+=l.mask.imageData.width*l.mask.imageData.height;if(pixels>33554432)throw Error('PSD decoded pixels exceed budget');result.mask={png:png(l.mask.imageData),left:l.mask.left??0,top:l.mask.top??0,width:l.mask.imageData.width,height:l.mask.imageData.height,default_color:l.mask.defaultColor??0};if(l.mask.positionRelativeToLayer){result.mask.left+=result.left;result.mask.top+=result.top;}if(l.mask.userMaskFeather||l.mask.userMaskDensity!==undefined)warnings.add('PSD mask feather/density is not reproduced; native mask pixels remain editable.');}
  for(const key of ['effects','adjustment','vectorMask','placedLayer','vectorFill','filterMask'])if(l[key])warnings.add(`PSD ${key} is retained in the original; source-rendered layer pixels are used where available.`);
  if(l.clipping)warnings.add('Clipping-chain PSD layers are not equivalent; inspect the imported composition.');
  return result;
 }
 if(!doc.children?.length){doc=ag.readPsd(bytes,{useImageData:true,skipThumbnail:true});if(doc.imageData){doc.children=[{name:'PSD composite',left:0,top:0,imageData:doc.imageData}];warnings.add('No separate layers; imported the source composite.');}}
 return{icc:doc.imageResources?.iccProfile?Buffer.from(doc.imageResources.iccProfile).toString("base64"):null,width:w,height:h,layers:(doc.children??[]).map(l=>walk(l,0)),warnings:[...warnings]};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){const result=decodePsd(await readSource(process.argv[2]));const text=JSON.stringify(result);if(Buffer.byteLength(text)>100663296)throw Error('PSD report exceeds 96 MiB');process.stdout.write(text);}
