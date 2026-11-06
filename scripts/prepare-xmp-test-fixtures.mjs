// Generates synthetic media only. Never opens the app's database or an installer.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { deflateSync } from 'node:zlib';

const args = process.argv.slice(2);
const verify = args[0] === '--verify';
if (args.length !== (verify ? 2 : 1)) {
  console.error('Usage: node scripts/prepare-xmp-test-fixtures.mjs [--verify] <directory>');
  process.exit(1);
}
const root = path.resolve(args[verify ? 1 : 0]);
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
if (verify) {
  const manifest = JSON.parse(fs.readFileSync(path.join(root, 'fixture-hashes.json'), 'utf8'));
  for (const file of manifest.files) {
    const target = path.resolve(root, file.path);
    const relative = path.relative(root, target);
    if (!relative || relative.startsWith('..') || path.isAbsolute(relative)) throw new Error('Unsafe fixture manifest path');
    // Do not follow file or ancestor links in a handoff directory.
    for (let current = target; ; current = path.dirname(current)) {
      if (fs.lstatSync(current).isSymbolicLink()) throw new Error('Linked fixture refused');
      if (current === root) break;
    }
    if (!fs.lstatSync(target).isFile() || sha256(fs.readFileSync(target)) !== file.sha256) throw new Error(`Changed or missing fixture: ${file.path}`);
  }
  console.log(`Verified ${manifest.files.length} fixture files. No files changed.`);
  process.exit(0);
}

// New directory only. Never merge into, overwrite or delete existing fixtures.
fs.mkdirSync(root);
const files = [];
function write(relative, bytes, role) {
  const target = path.join(root, relative);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, bytes, { flag: 'wx' });
  files.push({ path: relative, role, bytes: Buffer.byteLength(bytes), sha256: sha256(bytes) });
}
function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
}
function chunk(type, bytes) {
  const name = Buffer.from(type);
  const size = Buffer.alloc(4); size.writeUInt32BE(bytes.length);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(Buffer.concat([name, bytes])));
  return Buffer.concat([size, name, bytes, crc]);
}
function png(marker) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(2, 0); header.writeUInt32BE(2, 4);
  header[8] = 8; header[9] = 6; // RGBA, 8 bit, 2 by 2 pixels.
  const row = [0, 40, 140, 220, 255, 40, 140, 220, 255];
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', header),
    chunk('tEXt', Buffer.from(`Fixture\0${marker}`)),
    chunk('IDAT', deflateSync(Buffer.from([...row, ...row]))), chunk('IEND', Buffer.alloc(0)),
  ]);
}
function xmp(creator, tag) {
  return `<?xml version="1.0" encoding="UTF-8"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:fixture="urn:openphoto:test" xmp:CreatorTool="${creator}">
<dc:subject><rdf:Bag><rdf:li>${tag}</rdf:li></rdf:Bag></dc:subject><fixture:keep>Preserve this unrelated metadata</fixture:keep>
</rdf:Description></rdf:RDF></x:xmpmeta>
`;
}
for (const name of ['new-sidecar', 'managed-review', 'foreign-sidecar', 'source-change', 'legacy-recovery']) {
  write(`${name}/photo.png`, png('A'), 'synthetic original');
  if (name !== 'new-sidecar') write(`${name}/photo.png.xmp`, xmp(name === 'foreign-sidecar' ? 'Another editor' : 'Open-Photo', 'Fixture/Old'), 'initial sidecar');
}
write('source-change/replacement.png', png('B'), 'same-size synthetic replacement; not an original baseline');
write('legacy-recovery/photo.png.xmp.openphoto.bak', xmp('Open-Photo', 'Fixture/Recovery'), 'legacy recovery sentinel; must not be overwritten');
write('external-winner.xmp', xmp('Another editor', 'Fixture/ExternalWinner'), 'competing-editor sidecar for manual copy');
const instructions = `# Local XMP fixtures

All images are generated 2×2 PNGs. No personal media or application data is used.
These fixtures do not create an interrupted-operation database or prove installed acceptance.

Before testing, verify this directory with:
node scripts/prepare-xmp-test-fixtures.mjs --verify "<this directory>"
Verification is expected to fail after deliberate test changes. Generate a NEW directory to reset.

Add the individual case folders as workspace roots, not this entire directory.
- new-sidecar: add Fixture/New; a new photo.png.xmp should be created.
- managed-review: add Fixture/New; compare and Apply. Both tags and fixture:keep must survive.
- foreign-sidecar: add Fixture/New; the foreign sidecar must be skipped and preserved byte for byte.
- source-change: propose Fixture/New and inspect it. Then copy replacement.png OVER photo.png within this disposable case. Both PNGs are valid and equal in size. Apply must refuse; cancel and create a new proposal to proceed. The native regression suite additionally preserves the timestamp and checks file identity.
- legacy-recovery: propose a tag. The existing legacy backup must block Apply and remain untouched.
- competing editor: in managed-review, after comparison copy external-winner.xmp over photo.png.xmp. Stale Apply must refuse and preserve that winner. Inspect it again before Keep current or cancellation.

Compare original hashes before/after every case except the intentional source-change case. These are tiny deterministic inputs, not substitutes for real JPEG/HEIC/video or installed WebView2 testing. For actual interrupted saves/restoration, use the native process-exit tests and the separate disposable-Windows checklist; do not fabricate journal rows.
`;
write('README.md', instructions, 'instructions');
fs.writeFileSync(path.join(root, 'fixture-hashes.json'), JSON.stringify({ schema: 1, files }, null, 2) + '\n', { flag: 'wx' });
console.log(`Created ${files.length} synthetic fixture files in ${root}. Nothing installed or launched.`);
