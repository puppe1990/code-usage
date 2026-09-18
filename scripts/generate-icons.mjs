import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const name = Buffer.from(type, "latin1");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([name, data])), 0);
  return Buffer.concat([length, name, data, crc]);
}

function encodePng(width, height, rgba) {
  const stride = width * 4;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (stride + 1)] = 0;
    rgba.copy(raw, y * (stride + 1) + 1, y * stride, (y + 1) * stride);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

function gauge(x, y, size) {
  const center = size / 2;
  const radius = size * 0.34;
  const stroke = size * 0.09;
  const dx = x - center;
  const dy = y - center;
  const distance = Math.hypot(dx, dy);
  const angle = Math.atan2(dy, dx);

  const inBottomGap = angle > Math.PI / 2 - 0.45 && angle < Math.PI / 2 + 0.45;
  const ring = !inBottomGap && Math.abs(distance - radius) <= stroke / 2;

  const nx = Math.cos(-Math.PI / 4);
  const ny = Math.sin(-Math.PI / 4);
  const along = dx * nx + dy * ny;
  const across = Math.abs(-dx * ny + dy * nx);
  const needle = along >= 0 && along <= radius * 0.95 && across <= stroke * 0.45;

  const hub = distance <= stroke * 0.6;

  return ring || needle || hub ? 1 : 0;
}

function roundedRect(x, y, size, radius) {
  const dx = Math.max(radius - x, 0, x - (size - radius));
  const dy = Math.max(radius - y, 0, y - (size - radius));
  return Math.hypot(dx, dy) <= radius ? 1 : 0;
}

function coverage(x, y, size, shape) {
  let hits = 0;
  const step = 1 / 3;
  for (let sx = 0; sx < 3; sx++) {
    for (let sy = 0; sy < 3; sy++) {
      hits += shape(x + (sx + 0.5) * step, y + (sy + 0.5) * step, size);
    }
  }
  return hits / 9;
}

function render(size, { background, foreground }) {
  const rgba = Buffer.alloc(size * size * 4);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const index = (y * size + x) * 4;
      let alpha = 0;
      if (background) {
        const bg = coverage(x, y, size, (px, py, s) => roundedRect(px, py, s, s * 0.22));
        if (bg > 0) {
          rgba[index] = background[0];
          rgba[index + 1] = background[1];
          rgba[index + 2] = background[2];
          alpha = bg * 255;
        }
      }
      const fg = coverage(x, y, size, gauge);
      if (fg > 0) {
        rgba[index] = foreground[0];
        rgba[index + 1] = foreground[1];
        rgba[index + 2] = foreground[2];
        alpha = Math.max(alpha, fg * 255);
      }
      rgba[index + 3] = Math.round(alpha);
    }
  }
  return rgba;
}

const iconsDir = resolve(root, "src-tauri", "icons");
mkdirSync(iconsDir, { recursive: true });

writeFileSync(
  resolve(iconsDir, "source.png"),
  encodePng(1024, 1024, render(1024, { background: [13, 20, 33], foreground: [255, 255, 255] })),
);
writeFileSync(
  resolve(iconsDir, "tray-icon.png"),
  encodePng(44, 44, render(44, { background: null, foreground: [0, 0, 0] })),
);

console.log("icons written to", iconsDir);
