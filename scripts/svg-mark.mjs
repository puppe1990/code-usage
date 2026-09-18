import { readFileSync } from "node:fs";

/// Path commands used by the marks we rasterize (plus lineto, for completeness).
const NUMBER = /^[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?/;
const TAG = /<\/?g\b[^>]*>|<path\b[^>]*\/?>/gi;
const TAU = Math.PI * 2;

/// Character scanner for path data. Arc flags are single characters, so they cannot be read with a
/// number regex: in `a7.8 7.8 0 00-1.83-1` the flags are `0`,`0` and only then comes `-1.83`.
function createScanner(d) {
  let index = 0;

  const skip = () => {
    while (index < d.length && (d[index] === "," || /\s/.test(d[index]))) index++;
  };

  return {
    letter() {
      skip();
      return /[a-z]/i.test(d[index] ?? "") ? d[index++] : "";
    },
    number() {
      skip();
      const match = NUMBER.exec(d.slice(index));
      if (!match)
        throw new Error(`expected a number at ${index} in "${d.slice(index, index + 12)}"`);
      index += match[0].length;
      return Number(match[0]);
    },
    flag() {
      skip();
      if (d[index] !== "0" && d[index] !== "1") {
        throw new Error(`expected an arc flag at ${index} in "${d.slice(index, index + 12)}"`);
      }
      return Number(d[index++]);
    },
    done() {
      skip();
      return index >= d.length;
    },
  };
}

/// Reads an attribute from a tag, e.g. `fill="#fff"`.
function attribute(tag, name) {
  const match = tag.match(new RegExp(`\\b${name}="([^"]*)"`));
  return match?.[1];
}

/// Flattens an SVG path into closed polygons of line segments. The marks only use lines and cubic
/// beziers, and point-in-shape tests need no curve information afterwards.
function flattenPath(d, steps = 14) {
  const scanner = createScanner(d);
  const subpaths = [];
  let points = [];
  let command = "";
  let smooth = false;
  let x = 0;
  let y = 0;
  let startX = 0;
  let startY = 0;
  let controlX = 0;
  let controlY = 0;

  const readPoint = (relative) => {
    const px = scanner.number();
    const py = scanner.number();
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
  // endpoint parameterization -> center parameterization, per SVG spec F.6.5
  const ellipse = (from, rx, ry, rotation, largeArc, sweep, to) => {
    if (rx === 0 || ry === 0) {
      points.push(to);
      return;
    }
    const phi = (rotation * Math.PI) / 180;
    const cos = Math.cos(phi);
    const sin = Math.sin(phi);
    const dx = (from.x - to.x) / 2;
    const dy = (from.y - to.y) / 2;
    const x1 = cos * dx + sin * dy;
    const y1 = -sin * dx + cos * dy;

    const scaled = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if (scaled > 1) {
      const scale = Math.sqrt(scaled);
      rx *= scale;
      ry *= scale;
    }

    const numerator = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    const denominator = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    const coefficient =
      (largeArc === sweep ? -1 : 1) * Math.sqrt(Math.max(0, numerator / denominator));
    const centerX1 = (coefficient * rx * y1) / ry;
    const centerY1 = (-coefficient * ry * x1) / rx;
    const centerX = cos * centerX1 - sin * centerY1 + (from.x + to.x) / 2;
    const centerY = sin * centerX1 + cos * centerY1 + (from.y + to.y) / 2;

    const start = Math.atan2((y1 - centerY1) / ry, (x1 - centerX1) / rx);
    const end = Math.atan2((-y1 - centerY1) / ry, (-x1 - centerX1) / rx);
    let delta = end - start;
    if (sweep === 0 && delta > 0) delta -= TAU;
    if (sweep === 1 && delta < 0) delta += TAU;

    const count = Math.max(2, Math.ceil((Math.abs(delta) / TAU) * steps * 4));
    for (let step = 1; step <= count; step++) {
      const angle = start + (delta * step) / count;
      points.push({
        x: centerX + cos * rx * Math.cos(angle) - sin * ry * Math.sin(angle),
        y: centerY + sin * rx * Math.cos(angle) + cos * ry * Math.sin(angle),
      });
    }
  };

  while (!scanner.done()) {
    const letter = scanner.letter();
    if (letter) {
      command = letter;
    } else if (command === "M" || command === "m") {
      // repeated coordinates: M/m repeats as a lineto, the rest repeat themselves
      command = command === "M" ? "L" : "l";
    } else if (!command) {
      throw new Error("path data must start with a command");
    }
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
        x = relative ? x + scanner.number() : scanner.number();
        points.push({ x, y });
        break;
      }
      case "V": {
        y = relative ? y + scanner.number() : scanner.number();
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
        const control1 = smooth ? { x: 2 * x - controlX, y: 2 * y - controlY } : { x, y };
        const control2 = readPoint(relative);
        const end = readPoint(relative);
        bezier({ x, y }, control1, control2, end);
        controlX = control2.x;
        controlY = control2.y;
        ({ x, y } = end);
        break;
      }
      case "A": {
        const rx = scanner.number();
        const ry = scanner.number();
        const rotation = scanner.number();
        const largeArc = scanner.flag();
        const sweep = scanner.flag();
        const end = readPoint(relative);
        ellipse({ x, y }, rx, ry, rotation, largeArc, sweep, end);
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

    smooth = command.toUpperCase() === "C" || command.toUpperCase() === "S";
  }

  if (points.length > 0) subpaths.push(points);
  return subpaths;
}

/// Shapes that paint the mark: every path, except the ones explicitly filled with pure black — the
/// tile behind a brand mark, which would become a solid square in a template image — and any fill
/// the caller asks to skip. `<defs>`, `<mask>` and `<clipPath>` never paint directly, so their
/// paths are dropped.
function markShapes(svg, skip = []) {
  const markup = svg.replace(/<(defs|mask|clipPath)\b[\s\S]*?<\/\1>/gi, "");
  const skipped = ["#000", "#000000", "black", ...skip.map((fill) => fill.toLowerCase())];
  const root = markup.match(/<svg\b[^>]*>/i)?.[0] ?? "";
  const groups = [{ fill: attribute(root, "fill"), rule: attribute(root, "fill-rule") }];
  const shapes = [];

  for (const [tag] of markup.matchAll(TAG)) {
    if (tag.startsWith("</g")) {
      groups.pop();
      continue;
    }
    if (tag.startsWith("<g")) {
      if (!tag.endsWith("/>")) {
        groups.push({
          fill: attribute(tag, "fill") ?? groups.at(-1).fill,
          rule: attribute(tag, "fill-rule") ?? groups.at(-1).rule,
        });
      }
      continue;
    }

    const fill = (attribute(tag, "fill") ?? groups.at(-1).fill ?? "").toLowerCase();
    if (skipped.includes(fill)) continue;

    const polygons = flattenPath(attribute(tag, "d") ?? "");
    if (polygons.length > 0) {
      shapes.push({ polygons, rule: attribute(tag, "fill-rule") ?? groups.at(-1).rule });
    }
  }

  if (shapes.length === 0) throw new Error("no paintable paths found in the mark");
  return shapes;
}

/// Winding-number test (or even-odd parity when the shape asks for it).
function contains(shape, x, y) {
  let winding = 0;
  for (const points of shape.polygons) {
    for (let i = 0; i < points.length; i++) {
      const a = points[i];
      const b = points[(i + 1) % points.length];
      if (a.y > y === b.y > y) continue;
      const crossing = a.x + ((y - a.y) / (b.y - a.y)) * (b.x - a.x);
      if (crossing > x) winding += a.y < b.y ? 1 : -1;
    }
  }
  return shape.rule === "evenodd" ? winding % 2 !== 0 : winding !== 0;
}

/// Black mark on a transparent background, ready to be used as a macOS template image. The height
/// drives the size; the width follows the viewBox aspect ratio. `skip` lists fills to leave out.
export function renderMark(svgPath, height, { skip = [] } = {}) {
  const svg = readFileSync(svgPath, "utf8");
  const viewBox = (attribute(svg.match(/<svg\b[^>]*>/i)?.[0] ?? "", "viewBox") ?? "")
    .split(/\s+/)
    .map(Number);
  const width = Math.max(1, Math.round((height * viewBox[2]) / viewBox[3]));
  const shapes = markShapes(svg, skip);
  const rgba = Buffer.alloc(width * height * 4);
  const step = 1 / 3;

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      let hits = 0;
      for (let sx = 0; sx < 3; sx++) {
        for (let sy = 0; sy < 3; sy++) {
          const px = ((x + (sx + 0.5) * step) * viewBox[2]) / width + viewBox[0];
          const py = ((y + (sy + 0.5) * step) * viewBox[3]) / height + viewBox[1];
          if (shapes.some((shape) => contains(shape, px, py))) hits++;
        }
      }
      rgba[(y * width + x) * 4 + 3] = Math.round((hits / 9) * 255);
    }
  }

  return { width, height, rgba };
}
