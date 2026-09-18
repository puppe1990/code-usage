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

/// Cursor and output shared by the command handlers while a path is flattened.
function createPathState(scanner, steps) {
  return {
    scanner,
    steps,
    subpaths: [],
    points: [],
    command: "",
    x: 0,
    y: 0,
    startX: 0,
    startY: 0,
    controlX: 0,
    controlY: 0,
    smooth: false,
  };
}

/// Reads a coordinate pair; relative numbers are offset by the current cursor.
function readPoint(state, relative) {
  const x = state.scanner.number();
  const y = state.scanner.number();
  return relative ? { x: state.x + x, y: state.y + y } : { x, y };
}

function pushPoint(state, point) {
  state.points.push(point);
  state.x = point.x;
  state.y = point.y;
}

/// Samples a cubic bezier into `steps` line segments (including the end point).
function bezierPoints(from, control1, control2, to, steps) {
  const points = [];
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
  return points;
}

/// Endpoint parameterization -> center parameterization, per SVG spec F.6.5.
function arcCenter(from, to, rx, ry, rotation, largeArc, sweep) {
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

  const start = Math.atan2((y1 - centerY1) / ry, (x1 - centerX1) / rx);
  const end = Math.atan2((-y1 - centerY1) / ry, (-x1 - centerX1) / rx);
  let delta = end - start;
  if (sweep === 0 && delta > 0) delta -= TAU;
  if (sweep === 1 && delta < 0) delta += TAU;

  return {
    x: cos * centerX1 - sin * centerY1 + (from.x + to.x) / 2,
    y: sin * centerX1 + cos * centerY1 + (from.y + to.y) / 2,
    rx,
    ry,
    cos,
    sin,
    start,
    delta,
  };
}

/// Samples an elliptical arc into line segments (including the end point).
function arcPoints(from, to, rx, ry, rotation, largeArc, sweep, steps) {
  if (rx === 0 || ry === 0) return [to];

  const arc = arcCenter(from, to, rx, ry, rotation, largeArc, sweep);
  const count = Math.max(2, Math.ceil((Math.abs(arc.delta) / TAU) * steps * 4));
  const points = [];

  for (let step = 1; step <= count; step++) {
    const angle = arc.start + (arc.delta * step) / count;
    points.push({
      x: arc.x + arc.cos * arc.rx * Math.cos(angle) - arc.sin * arc.ry * Math.sin(angle),
      y: arc.y + arc.sin * arc.rx * Math.cos(angle) + arc.cos * arc.ry * Math.sin(angle),
    });
  }
  return points;
}

function startSubpath(state, relative) {
  if (state.points.length > 0) state.subpaths.push(state.points);
  state.points = [];

  const point = readPoint(state, relative);
  state.startX = point.x;
  state.startY = point.y;
  state.controlX = point.x;
  state.controlY = point.y;
  pushPoint(state, point);
}

function lineTo(state, relative) {
  pushPoint(state, readPoint(state, relative));
}

function horizontalTo(state, relative) {
  const x = state.scanner.number();
  pushPoint(state, { x: relative ? state.x + x : x, y: state.y });
}

function verticalTo(state, relative) {
  const y = state.scanner.number();
  pushPoint(state, { x: state.x, y: relative ? state.y + y : y });
}

function drawCurve(state, control1, control2, end) {
  state.points.push(
    ...bezierPoints({ x: state.x, y: state.y }, control1, control2, end, state.steps),
  );
  state.controlX = control2.x;
  state.controlY = control2.y;
  state.x = end.x;
  state.y = end.y;
}

function curveTo(state, relative) {
  const control1 = readPoint(state, relative);
  const control2 = readPoint(state, relative);
  drawCurve(state, control1, control2, readPoint(state, relative));
}

function smoothCurveTo(state, relative) {
  const control1 = state.smooth
    ? { x: 2 * state.x - state.controlX, y: 2 * state.y - state.controlY }
    : { x: state.x, y: state.y };
  const control2 = readPoint(state, relative);
  drawCurve(state, control1, control2, readPoint(state, relative));
}

function arcTo(state, relative) {
  const rx = state.scanner.number();
  const ry = state.scanner.number();
  const rotation = state.scanner.number();
  const largeArc = state.scanner.flag();
  const sweep = state.scanner.flag();
  const end = readPoint(state, relative);

  state.points.push(
    ...arcPoints({ x: state.x, y: state.y }, end, rx, ry, rotation, largeArc, sweep, state.steps),
  );
  state.x = end.x;
  state.y = end.y;
}

function closeSubpath(state) {
  if (state.points.length > 0) state.subpaths.push(state.points);
  state.points = [];
  state.x = state.startX;
  state.y = state.startY;
}

/// Runs one already-resolved path command against the shared cursor.
function drawCommand(state, command) {
  const relative = command === command.toLowerCase();

  switch (command.toUpperCase()) {
    case "M":
      startSubpath(state, relative);
      break;
    case "L":
      lineTo(state, relative);
      break;
    case "H":
      horizontalTo(state, relative);
      break;
    case "V":
      verticalTo(state, relative);
      break;
    case "C":
      curveTo(state, relative);
      break;
    case "S":
      smoothCurveTo(state, relative);
      break;
    case "A":
      arcTo(state, relative);
      break;
    case "Z":
      closeSubpath(state);
      break;
    default:
      throw new Error(`unsupported path command: ${command}`);
  }
}

/// Coordinates with no letter repeat the previous command; M/m repeat as a lineto.
function resolveCommand(previous, letter) {
  if (letter) return letter;
  if (!previous) throw new Error("path data must start with a command");
  if (previous === "M") return "L";
  if (previous === "m") return "l";
  return previous;
}

/// Flattens an SVG path into closed polygons of line segments. The marks only use lines, cubic
/// beziers and elliptical arcs, and point-in-shape tests need no curve information afterwards.
export function flattenPath(d, steps = 14) {
  const scanner = createScanner(d);
  const state = createPathState(scanner, steps);

  while (!scanner.done()) {
    state.command = resolveCommand(state.command, scanner.letter());
    drawCommand(state, state.command);
    state.smooth = state.command.toUpperCase() === "C" || state.command.toUpperCase() === "S";
  }

  if (state.points.length > 0) state.subpaths.push(state.points);
  return state.subpaths;
}

/// Paint (fill and fill-rule) a group passes down to its children.
function groupPaint(tag) {
  return { fill: attribute(tag, "fill"), rule: attribute(tag, "fill-rule") };
}

/// Paint of a nested group: its own attributes, or whatever the parent already had.
function mergedPaint(parent, tag) {
  return {
    fill: attribute(tag, "fill") ?? parent.fill,
    rule: attribute(tag, "fill-rule") ?? parent.rule,
  };
}

/// Path geometry plus fill rule, or null when the path is a tile/background fill the caller skips.
function pathShape(tag, paint, skipped) {
  const fill = (attribute(tag, "fill") ?? paint.fill ?? "").toLowerCase();
  if (skipped.includes(fill)) return null;

  const polygons = flattenPath(attribute(tag, "d") ?? "");
  if (polygons.length === 0) return null;
  return { polygons, rule: attribute(tag, "fill-rule") ?? paint.rule };
}

/// Shapes that paint the mark: every path, except the ones explicitly filled with pure black — the
/// tile behind a brand mark, which would become a solid square in a template image — and any fill
/// the caller asks to skip. `<defs>`, `<mask>` and `<clipPath>` never paint directly, so their
/// paths are dropped.
function markShapes(svg, skip = []) {
  const markup = svg.replace(/<(defs|mask|clipPath)\b[\s\S]*?<\/\1>/gi, "");
  const skipped = ["#000", "#000000", "black", ...skip.map((fill) => fill.toLowerCase())];
  const groups = [groupPaint(markup.match(/<svg\b[^>]*>/i)?.[0] ?? "")];
  const shapes = [];

  for (const [tag] of markup.matchAll(TAG)) {
    if (tag.startsWith("<g")) {
      if (!tag.endsWith("/>")) groups.push(mergedPaint(groups.at(-1), tag));
      continue;
    }
    if (tag.startsWith("</g")) {
      groups.pop();
      continue;
    }

    const shape = pathShape(tag, groups.at(-1), skipped);
    if (shape) shapes.push(shape);
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

const SAMPLE_GRID = 3;
const SAMPLE_STEP = 1 / SAMPLE_GRID;

/// Fraction of a pixel covered by the mark, by supersampling a 3x3 grid.
function pixelCoverage(shapes, viewBox, width, height, x, y) {
  let hits = 0;
  for (let sx = 0; sx < SAMPLE_GRID; sx++) {
    for (let sy = 0; sy < SAMPLE_GRID; sy++) {
      const px = ((x + (sx + 0.5) * SAMPLE_STEP) * viewBox[2]) / width + viewBox[0];
      const py = ((y + (sy + 0.5) * SAMPLE_STEP) * viewBox[3]) / height + viewBox[1];
      if (shapes.some((shape) => contains(shape, px, py))) hits++;
    }
  }
  return hits / (SAMPLE_GRID * SAMPLE_GRID);
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

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const coverage = pixelCoverage(shapes, viewBox, width, height, x, y);
      rgba[(y * width + x) * 4 + 3] = Math.round(coverage * 255);
    }
  }

  return { width, height, rgba };
}
