// Modern sectioned-container bridge, using the shipped JS zlib/zstd readers.
import {readSource} from './astral-source-input.mjs';
import path from 'node:path';
import {decodeDocument,normalizeDecodedDocument,verifyEmbeddedSource} from '../src/native-container.mjs';
import {validateScene} from '../src/pivot/model.mjs';
const filename=process.argv[2],kind=path.extname(filename).slice(1).toLowerCase();
if(!['styk','stkz','stykmov'].includes(kind))throw new Error('Unsupported native container extension');
const bytes=await readSource(filename);if(bytes.length>128*1024*1024)throw new Error('Legacy source exceeds 128 MiB');
const decoded=await decodeDocument(bytes,kind),document=normalizeDecodedDocument(decoded.document,kind);
validateScene(document);await verifyEmbeddedSource(document);
const text=JSON.stringify(document);if(Buffer.byteLength(text)>256*1024*1024)throw new Error('Decoded document exceeds 256 MiB');
process.stdout.write(text);
