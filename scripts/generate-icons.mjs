import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';

function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) {
    c ^= buf[i];
    for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (c & 1 ? 0xedb88320 : 0);
  }
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const t = Buffer.from(type, 'ascii');
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([t, data])), 0);
  return Buffer.concat([len, t, data, crc]);
}

function makePng(size) {
  const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  const rowLen = 1 + size * 4;
  const raw = Buffer.alloc(rowLen * size);
  const pad = Math.max(2, Math.floor(size / 8));
  const stroke = Math.max(2, Math.floor(size / 7));

  for (let y = 0; y < size; y++) {
    const off = y * rowLen;
    raw[off] = 0;
    for (let x = 0; x < size; x++) {
      const p = off + 1 + x * 4;
      const isBorder = x < 2 || y < 2 || x >= size - 2 || y >= size - 2;
      const inInner = x >= pad && x < size - pad && y >= pad && y < size - pad;
      const isLeftBar = inInner && x < pad + stroke;
      const isRightBar = inInner && x >= size - pad - stroke;
      const diagDist = Math.abs((x - pad) - (y - pad));
      const isDiag = inInner && diagDist < stroke * 0.85;

      if (isBorder) {
        raw[p] = 99;
        raw[p + 1] = 102;
        raw[p + 2] = 241;
        raw[p + 3] = 255;
      } else if (isLeftBar || isRightBar || isDiag) {
        raw[p] = 248;
        raw[p + 1] = 250;
        raw[p + 2] = 252;
        raw[p + 3] = 255;
      } else {
        raw[p] = 24;
        raw[p + 1] = 24;
        raw[p + 2] = 36;
        raw[p + 3] = 255;
      }
    }
  }
  const idat = zlib.deflateSync(raw);
  return Buffer.concat([sig, chunk('IHDR', ihdr), chunk('IDAT', idat), chunk('IEND', Buffer.alloc(0))]);
}

const dir = 'apps/app/src-tauri/icons';
fs.mkdirSync(dir, { recursive: true });
const png32 = makePng(32);
const png128 = makePng(128);
const png256 = makePng(256);

fs.writeFileSync(path.join(dir, '32x32.png'), png32);
fs.writeFileSync(path.join(dir, '128x128.png'), png128);
fs.writeFileSync(path.join(dir, '128x128@2x.png'), png256);
fs.writeFileSync(path.join(dir, 'icon.png'), png128);

const icoHeader = Buffer.alloc(6);
icoHeader.writeUInt16LE(0, 0);
icoHeader.writeUInt16LE(1, 2);
icoHeader.writeUInt16LE(1, 4);
const icoEntry = Buffer.alloc(16);
icoEntry[0] = 32;
icoEntry[1] = 32;
icoEntry[2] = 0;
icoEntry[3] = 0;
icoEntry.writeUInt16LE(1, 4);
icoEntry.writeUInt16LE(32, 6);
icoEntry.writeUInt32LE(png32.length, 8);
icoEntry.writeUInt32LE(22, 12);
fs.writeFileSync(path.join(dir, 'icon.ico'), Buffer.concat([icoHeader, icoEntry, png32]));
console.log('Generated app icons in', dir);
