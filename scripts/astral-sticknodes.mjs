// Read-only compatibility bridge; uses the same adapter as the legacy editor.
import {readSource} from './astral-source-input.mjs';
import path from 'node:path';
import {importStickNodes} from '../src/sticknodes/adapter.mjs';
const filename=process.argv[2];
const bytes=await readSource(filename);
if(bytes.length>128*1024*1024)throw new Error('Legacy source exceeds 128 MiB');
const result=await importStickNodes(bytes,{filename:path.basename(filename)});
process.stdout.write(JSON.stringify(result.document??result.report??result));
