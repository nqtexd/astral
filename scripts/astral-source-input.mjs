// All decoders consume one bounded immutable snapshot supplied by the Rust importer.
import {createReadStream} from 'node:fs';
export async function readSource(filename,fromStdin=process.argv.includes('--stdin')){
  const chunks=[];let size=0;
  for await(const chunk of fromStdin?process.stdin:createReadStream(filename)){
    size+=chunk.length;if(size>128*1024*1024)throw new Error('Legacy source exceeds 128 MiB');chunks.push(chunk);
  }
  return Buffer.concat(chunks);
}
