// Mesmo valor de --hand-jitter em styles/tokens.css: o SVG não lê token CSS ao gerar o path.
export const DEFAULT_JITTER = 1.2

const SECOND_PASS_JITTER_RATIO = 0.8
const OVERSHOOT_RATIO = 0.04
const ELLIPSE_POINTS = 8
const ELLIPSE_TURN = 1.12
const ELLIPSE_RADIUS_NOISE = 0.5
const CATMULL_TENSION = 6
const ARROW_HEAD_MAX = 8
const ARROW_HEAD_RATIO = 0.35
const ARROW_HEAD_ANGLE = Math.PI / 7
const SCRIBBLE_STROKES = 5
const QUADRATIC_SAMPLES = 8
const DECIMALS = 2

type Rand = () => number

interface Point {
  x: number
  y: number
}

// Determinístico: a mesma seed repete o traço entre renders, então ele não "treme".
function mulberry32(seed: number): Rand {
  let state = seed >>> 0
  return () => {
    state = (state + 0x6d2b79f5) >>> 0
    let t = state
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

const finite = (value: number) => (Number.isFinite(value) ? value : 0)
const size = (value: number) => Math.max(0, finite(value))
const num = (value: number) => finite(value).toFixed(DECIMALS)
const point = (x: number, y: number) => `${num(x)} ${num(y)}`
const noise = (rand: Rand, jitter: number) => (rand() * 2 - 1) * jitter
const seeded = (seed: number) => mulberry32(Math.trunc(finite(seed)))

function jittered(rand: Rand, x: number, y: number, jitter: number): Point {
  return { x: x + noise(rand, jitter), y: y + noise(rand, jitter) }
}

function cubic(rand: Rand, from: Point, to: Point, jitter: number): string {
  const control = (ratio: number) =>
    point(
      from.x + (to.x - from.x) * ratio + noise(rand, jitter),
      from.y + (to.y - from.y) * ratio + noise(rand, jitter),
    )
  return `C${control(1 / 3)} ${control(2 / 3)} ${point(to.x, to.y)}`
}

function stroke(rand: Rand, points: Point[], jitter: number): string {
  const [first, ...rest] = points
  const segments = rest.map((to, index) => cubic(rand, points[index], to, jitter))
  return `M${point(first.x, first.y)}${segments.join('')}`
}

function polygonPass(rand: Rand, vertices: Point[], jitter: number): string {
  const corners = vertices.map((vertex) => jittered(rand, vertex.x, vertex.y, jitter))
  const [start, nextCorner] = corners
  const overshoot = {
    x: start.x + (nextCorner.x - start.x) * OVERSHOOT_RATIO,
    y: start.y + (nextCorner.y - start.y) * OVERSHOOT_RATIO,
  }
  return stroke(rand, [...corners, start, overshoot], jitter)
}

export function sketchLine(x1: number, y1: number, x2: number, y2: number, seed: number, jitter: number): string {
  const rand = seeded(seed)
  const amount = size(jitter)
  return stroke(rand, [jittered(rand, x1, y1, amount / 2), jittered(rand, x2, y2, amount / 2)], amount)
}

// Duas passadas levemente deslocadas: o traço duplo do lápis.
export function sketchRect(w: number, h: number, seed: number, jitter: number): string {
  const rand = seeded(seed)
  const amount = size(jitter)
  const width = size(w)
  const height = size(h)
  const corners = [
    { x: 0, y: 0 },
    { x: width, y: 0 },
    { x: width, y: height },
    { x: 0, y: height },
  ]
  return polygonPass(rand, corners, amount) + polygonPass(rand, corners, amount * SECOND_PASS_JITTER_RATIO)
}

// Losango inscrito na caixa w×h, com as duas passadas do lápis como no retângulo.
export function sketchDiamond(w: number, h: number, seed: number, jitter: number): string {
  const rand = seeded(seed)
  const amount = size(jitter)
  const width = size(w)
  const height = size(h)
  const corners = [
    { x: width / 2, y: 0 },
    { x: width, y: height / 2 },
    { x: width / 2, y: height },
    { x: 0, y: height / 2 },
  ]
  return polygonPass(rand, corners, amount) + polygonPass(rand, corners, amount * SECOND_PASS_JITTER_RATIO)
}

// Curva quadrática amostrada e tremida: as pontas ficam onde estão para a seta continuar encostando no nó.
export function sketchQuadratic(from: Point, control: Point, to: Point, seed: number, jitter: number): string {
  const rand = seeded(seed)
  const amount = size(jitter)
  const points: Point[] = []
  for (let index = 0; index <= QUADRATIC_SAMPLES; index += 1) {
    const t = index / QUADRATIC_SAMPLES
    const x = (1 - t) ** 2 * finite(from.x) + 2 * (1 - t) * t * finite(control.x) + t ** 2 * finite(to.x)
    const y = (1 - t) ** 2 * finite(from.y) + 2 * (1 - t) * t * finite(control.y) + t ** 2 * finite(to.y)
    const atEnd = index === 0 || index === QUADRATIC_SAMPLES
    points.push(atEnd ? { x, y } : jittered(rand, x, y, amount / 2))
  }
  return stroke(rand, points, amount / 2)
}

export function sketchEllipse(w: number, h: number, seed: number, jitter: number): string {
  const rand = seeded(seed)
  const amount = size(jitter)
  const rx = size(w) / 2
  const ry = size(h) / 2
  const start = rand() * Math.PI * 2
  const points: Point[] = []
  for (let index = 0; index <= ELLIPSE_POINTS; index += 1) {
    const angle = start + (index / ELLIPSE_POINTS) * Math.PI * 2 * ELLIPSE_TURN
    const wobble = 1 + noise(rand, amount * ELLIPSE_RADIUS_NOISE) / Math.max(rx, ry, 1)
    points.push({ x: rx + Math.cos(angle) * rx * wobble, y: ry + Math.sin(angle) * ry * wobble })
  }
  const segments = points.slice(0, -1).map((current, index) => {
    const before = points[Math.max(index - 1, 0)]
    const next = points[index + 1]
    const after = points[Math.min(index + 2, points.length - 1)]
    const c1 = { x: current.x + (next.x - before.x) / CATMULL_TENSION, y: current.y + (next.y - before.y) / CATMULL_TENSION }
    const c2 = { x: next.x - (after.x - current.x) / CATMULL_TENSION, y: next.y - (after.y - current.y) / CATMULL_TENSION }
    return `C${point(c1.x, c1.y)} ${point(c2.x, c2.y)} ${point(next.x, next.y)}`
  })
  return `M${point(points[0].x, points[0].y)}${segments.join('')}`
}

export function sketchArrow(x1: number, y1: number, x2: number, y2: number, seed: number, jitter: number): string {
  const shaft = sketchLine(x1, y1, x2, y2, seed, jitter)
  const dx = finite(x2) - finite(x1)
  const dy = finite(y2) - finite(y1)
  const length = Math.hypot(dx, dy)
  if (length === 0) {
    return shaft
  }
  const head = Math.min(length * ARROW_HEAD_RATIO, ARROW_HEAD_MAX)
  const back = Math.atan2(dy, dx) + Math.PI
  const wing = (turn: number, offset: number) =>
    sketchLine(x2, y2, x2 + Math.cos(back + turn) * head, y2 + Math.sin(back + turn) * head, seed + offset, jitter)
  return shaft + wing(ARROW_HEAD_ANGLE, 1) + wing(-ARROW_HEAD_ANGLE, 2)
}

export function sketchCheck(w: number, h: number, seed: number, jitter: number): string {
  const width = size(w)
  const height = size(h)
  return (
    sketchLine(width * 0.1, height * 0.55, width * 0.4, height * 0.9, seed, jitter) +
    sketchLine(width * 0.4, height * 0.9, width * 0.95, height * 0.1, seed + 1, jitter)
  )
}

export function sketchStrike(w: number, h: number, seed: number, jitter: number): string {
  const width = size(w)
  const height = size(h)
  return (
    sketchLine(0, height * 0.6, width, height * 0.4, seed, jitter) +
    sketchLine(0, height * 0.5, width, height * 0.55, seed + 1, jitter)
  )
}

export function sketchScribble(w: number, h: number, seed: number, jitter: number): string {
  const width = size(w)
  const height = size(h)
  const step = width / SCRIBBLE_STROKES
  let path = ''
  for (let index = 0; index < SCRIBBLE_STROKES; index += 1) {
    const top = index % 2 === 0
    path += sketchLine(step * index, top ? 0 : height, step * (index + 1), top ? height : 0, seed + index, jitter)
  }
  return path
}
