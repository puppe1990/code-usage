import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const glyphSource = resolve(root, "src-tauri", "icons", "command-code.svg");

/// Path commands used by the mark (plus lineto, for completeness).
const PATH_TOKEN = /([MmLlHhVvCcSsZz])|(-?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?)/g;

/// Flattens an SVG path into closed polygons of line segments. The mark only uses lines and cubic
/// beziers, and even-odd ray casting needs no curve information afterwards.
export function flattenPath(d, steps = 14) {
  const input = [...d.matchAll(PATH_TOKEN)].map((match) =>
    match[1] === undefined ? Number(match[2]) : match[1],
  );
  const subpaths = [];
  let points = [];
  let repeated = "";
  let index = 0;
  let x = 0;
  let y = 0;
  let startX = 0;
  let startY = 0;
  let controlX = 0;
  let controlY = 0;

  const number = () => Number(input[index++]);
  const readPoint = (relative) => {
    const px = number();
    const py = number();
    return { x: relative ? x + px : px, y: relative ? y + py : py };
  };
  const bezier = (from, control1, control2, to) => {
    for (let step = 1; step <= steps; step++) {
      const t = step / steps;
      const rest = 1 - t;
      const a = rest * rest * rest;
      const b = 3 * rest * rest * t;
      const c = 3 * rest * t * t;
      const d = t * t * t;
      points.push({
        x: a * from.x + b * control1.x + c * control2.x + d * to.x,
        y: a * from.y + b * control1.y + c * control2.y + d * to.y,
      });
    }
  };

  while (index < input.length) {
    const token = input[index];
    let command = token;
    if (typeof token === "string") {
      index++;
    } else {
      // repeated coordinates: M/m repeats as a lineto, the rest repeat themselves
      command = repeated === "M" ? "L" : repeated === "m" ? "l" : repeated;
    }
    repeated = command;
    const relative = command === command.toLowerCase();

    switch (command.toUpperCase()) {
      case "M": {
        if (points.length > 0) subpaths.push(points);
        points = [];
        const point = readPoint(relative);
        ({ x, y } = point);
        startX = x;
        startY = y;
        controlX = x;
        controlY = y;
        points.push(point);
        break;
      }
      case "L": {
        const point = readPoint(relative);
        ({ x, y } = point);
        points.push(point);
        break;
      }
      case "H": {
        x = relative ? x + number() : number();
        points.push({ x, y });
        break;
      }
      case "V": {
        y = relative ? y + number() : number();
        points.push({ x, y });
        break;
      }
      case "C": {
        const control1 = readPoint(relative);
        const control2 = readPoint(relative);
        const end = readPoint(relative);
        bezier({ x, y }, control1, control2, end);
        controlX = control2.x;
        controlY = control2.y;
        ({ x, y } = end);
        break;
      }
      case "S": {
        const control1 = { x: 2 * x - controlX, y: 2 * y - controlY };
        const control2 = readPoint(relative);
        const end = readPoint(relative);
        bezier({ x, y }, control1, control2, end);
        controlX = control2.x;
        controlY = control2.y;
        ({ x, y } = end);
        break;
      }
      case "Z": {
        if (points.length > 0) subpaths.push(points);
        points = [];
        x = startX;
        y = startY;
        break;
      }
      default:
        throw new Error(`unsupported path command: ${command}`);
    }
  }

  if (points.length > 0) subpaths.push(points);
  return subpaths;
}

/// The tray mark is the glyph group of `command-code.svg` (its white paths): a template image only
/// uses the alpha channel, so the black tile behind the glyph would become a solid square.
export function markGlyph(path = glyphSource) {
  const svg = readFileSync(path, "utf8");
  const group = svg.match(/<g[^>]*>([\s\S]*?)<\/g>/);
  if (!group) throw new Error("command-code.svg has no glyph group");

  const polygons = [...group[1].matchAll(/ d="([^"]+)"/g)].flatMap((match) =>
    flattenPath(match[1]),
  );
  if (polygons.length === 0) throw new Error("command-code.svg has no glyph paths");
  return polygons;
}

/// Even-odd ray casting: nested contours (the tile border, the rings) become holes.
export function insideGlyph(polygons, x, y) {
  let inside = false;
  for (const points of polygons) {
    for (let i = 0; i < points.length; i++) {
      const a = points[i];
      const b = points[(i + 1) % points.length];
      if (a.y > y !== b.y > y) {
        const crossing = a.x + ((y - a.y) / (b.y - a.y)) * (b.x - a.x);
        if (crossing > x) inside = !inside;
      }
    }
  }
  return inside;
}

const VIEW_BOX = 137;

/// Black mark on a transparent background, ready to be used as a macOS template image.
export function renderCommandCode(size, { file = glyphSource } = {}) {
  const glyph = markGlyph(file);
  const scale = VIEW_BOX / size;
  const rgba = Buffer.alloc(size * size * 4);
  const step = 1 / 3;

  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      let hits = 0;
      for (let sx = 0; sx < 3; sx++) {
        for (let sy = 0; sy < 3; sy++) {
          const px = (x + (sx + 0.5) * step) * scale;
          const py = (y + (sy + 0.5) * step) * scale;
          if (insideGlyph(glyph, px, py)) hits++;
        }
      }
      rgba[(y * size + x) * 4 + 3] = Math.round((hits / 9) * 255);
    }
  }

  return rgba;
}
