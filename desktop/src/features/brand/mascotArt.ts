export type MascotPose = 'feliz' | 'piscando' | 'animado' | 'de-costas'
export type MascotState =
  | 'trabalhando'
  | 'precisa-de-voce'
  | 'tudo-em-dia'
  | 'erro'
  | 'pausado'
  | 'atualizacao-pronta'
export type MascotLook = MascotPose | MascotState
export type MascotHead = 'cor' | 'preto'
export type MascotPart = 'fumaca' | 'bracos-cima' | 'brasa' | 'aceno'

type MascotToken = 'ink' | 'surface'
export type MascotPaint = MascotToken | 'brand' | 'currentColor' | 'none' | `#${string}`

export interface MascotShape {
  tag: 'path' | 'ellipse' | 'circle' | 'rect'
  attrs: Record<string, string | number>
  fill?: MascotPaint
  stroke?: MascotPaint
  part?: MascotPart
}

export interface MascotArt {
  viewBox: string
  parts: MascotShape[]
}

type Point = readonly [number, number]

const BODY_VIEWBOX = '0 -8 64 88'
const HEAD_VIEWBOX = '0 0 64 64'

const FLAME_BODY_D =
  'M32 4 C 34 11, 38 14, 42 19 C 49 27, 52 33, 52 40 C 52 52, 43 60, 32 60 C 21 60, 12 52, 12 40 C 12 31, 18 24, 23 18 C 24 23, 26 25, 29 26 C 27 19, 28 11, 32 4 Z'
const FLAME_TUFT_D = 'M36 8 C 41 11, 43 6, 42 2 C 46 6, 46 13, 40 16 Z'

function body(parts: MascotShape[]): MascotArt {
  return { viewBox: BODY_VIEWBOX, parts }
}

function head(parts: MascotShape[]): MascotArt {
  return { viewBox: HEAD_VIEWBOX, parts }
}

function stroked(d: string, stroke: MascotPaint, width: number, part?: MascotPart): MascotShape {
  return {
    tag: 'path',
    attrs: { d, 'stroke-width': width, 'stroke-linecap': 'round' },
    fill: 'none',
    stroke,
    part,
  }
}

function hand(cx: number, cy: number, r: number, part?: MascotPart): MascotShape {
  return { tag: 'circle', attrs: { cx, cy, r }, fill: 'ink', part }
}

function groundShadow(cy: number, rx: number): MascotShape {
  return { tag: 'ellipse', attrs: { cx: 32, cy, rx, ry: 3, opacity: 0.12 }, fill: '#000' }
}

function flame(paint: MascotPaint): MascotShape[] {
  return [
    { tag: 'path', attrs: { d: FLAME_BODY_D }, fill: paint },
    { tag: 'path', attrs: { d: FLAME_TUFT_D }, fill: paint },
  ]
}

function openEye(cx: number, pupil: Point, face: MascotPaint, glint?: Point): MascotShape[] {
  const shapes: MascotShape[] = [
    { tag: 'ellipse', attrs: { cx, cy: 40, rx: 3.6, ry: 4.4 }, fill: face },
    { tag: 'circle', attrs: { cx: pupil[0], cy: pupil[1], r: 2 }, fill: '#1c1c21' },
  ]
  if (glint) {
    shapes.push({ tag: 'circle', attrs: { cx: glint[0], cy: glint[1], r: 0.8 }, fill: face })
  }
  return shapes
}

function smile(face: MascotPaint): MascotShape {
  return stroked('M28 48.5 Q32 53 36 48.5', face, 2)
}

function wink(face: MascotPaint): MascotShape {
  return stroked('M35.5 40.5 Q39 37 42.5 40.5', face, 2)
}

const CHEEKS: MascotShape[] = [
  { tag: 'ellipse', attrs: { cx: 22, cy: 44, rx: 3, ry: 1.6, opacity: 0.45 }, fill: '#ff8f8f' },
  { tag: 'ellipse', attrs: { cx: 42, cy: 44, rx: 3, ry: 1.6, opacity: 0.45 }, fill: '#ff8f8f' },
]

const LEG_SHAPES: MascotShape[] = [
  stroked('M25 58 V71 M39 58 V71', 'ink', 3.5),
  {
    tag: 'path',
    attrs: { d: 'M20 74 C 20 71, 24 70, 26 70 H30 V74 Z M34 74 V70 H38 C 40 70, 44 71, 44 74 Z' },
    fill: 'ink',
  },
]

const FOOT_SHINE: MascotShape = {
  tag: 'path',
  attrs: { d: 'M21 72 H29', 'stroke-width': 1, opacity: 0.55 },
  fill: 'none',
  stroke: 'surface',
}

const LEGS: MascotShape[] = [...LEG_SHAPES, FOOT_SHINE]

const ARMS_DOWN: MascotShape[] = [
  stroked('M13 44 C 8 46, 7 50, 8 54 M51 44 C 56 46, 57 50, 56 54', 'ink', 3.5),
  hand(8, 55, 3.2),
  hand(56, 55, 3.2),
]

function armsUp(part?: MascotPart): MascotShape[] {
  return [
    stroked('M13 42 C 8 38, 6 33, 7 28 M51 42 C 56 38, 58 33, 57 28', 'ink', 3.5, part),
    hand(7, 26, 3.4, part),
    hand(57, 26, 3.4, part),
  ]
}

const HAPPY_EYES: MascotShape = {
  tag: 'path',
  attrs: {
    d: 'M21.5 41 L25 37.5 L28.5 41 M35.5 41 L39 37.5 L42.5 41',
    'stroke-width': 2.2,
    'stroke-linecap': 'round',
    'stroke-linejoin': 'round',
  },
  fill: 'none',
  stroke: 'surface',
}

const OPEN_MOUTH: MascotShape = { tag: 'path', attrs: { d: 'M27.5 48 Q32 55 36.5 48 Z' }, fill: 'surface' }

function raisedArmsShapes(part?: MascotPart): MascotShape[] {
  return [
    groundShadow(76, 20),
    ...LEGS,
    ...armsUp(part),
    ...flame('ink'),
    ...CHEEKS,
    HAPPY_EYES,
    OPEN_MOUTH,
  ]
}

const NOTEBOOK: MascotShape[] = [
  {
    tag: 'rect',
    attrs: { x: 19, y: 50, width: 26, height: 15, rx: 2, 'stroke-width': 1.2 },
    fill: '#2f2f36',
    stroke: 'surface',
  },
  { tag: 'circle', attrs: { cx: 32, cy: 57.5, r: 1.8, opacity: 0.8 }, fill: 'surface' },
  { tag: 'rect', attrs: { x: 15, y: 64, width: 34, height: 3, rx: 1.5 }, fill: '#2f2f36' },
]

const FELIZ_SHAPES: MascotShape[] = [
  groundShadow(76, 20),
  ...LEGS,
  ...ARMS_DOWN,
  ...flame('ink'),
  ...CHEEKS,
  ...openEye(25, [25.6, 40.4], 'surface', [26.5, 38.6]),
  ...openEye(39, [39.6, 40.4], 'surface', [40.5, 38.6]),
  smile('surface'),
]

const TRABALHANDO_SHAPES: MascotShape[] = [
  groundShadow(76, 20),
  ...LEGS,
  stroked('M14 46 C 14 52, 18 55, 22 55 M50 46 C 50 52, 46 55, 42 55', 'ink', 3.5),
  ...flame('ink'),
  ...CHEEKS,
  ...openEye(25, [25.6, 40.4], 'surface', [26.5, 38.6]),
  ...openEye(39, [39.6, 40.4], 'surface', [40.5, 38.6]),
  smile('surface'),
  ...NOTEBOOK,
]

const PRECISA_DE_VOCE_SHAPES: MascotShape[] = [
  groundShadow(76, 20),
  ...LEGS,
  stroked('M13 44 C 8 46, 6 50, 6 54', 'ink', 3.5),
  hand(6, 55, 3.2),
  stroked('M51 42 C 56 38, 58 33, 57 28', 'ink', 3.5, 'aceno'),
  hand(57, 26, 3.4, 'aceno'),
  ...flame('ink'),
  ...CHEEKS,
  ...openEye(25, [25.8, 40.4], 'surface'),
  wink('surface'),
  smile('surface'),
]

const ERRO_SHAPES: MascotShape[] = [
  groundShadow(76, 20),
  ...LEGS,
  ...ARMS_DOWN,
  stroked('M44 6 C 40 2, 46 -2, 42 -6 M48 10 C 46 6, 51 3, 48 -1', '#a6a6b0', 2, 'fumaca'),
  {
    tag: 'path',
    attrs: {
      d: 'M32 14 C 34 19, 38 22, 42 26 C 49 32, 52 36, 52 42 C 52 53, 43 60, 32 60 C 21 60, 12 53, 12 42 C 12 34, 18 28, 22 24 C 24 28, 27 29, 29 30 C 28 25, 29 19, 32 14 Z',
    },
    fill: 'ink',
  },
  ...openEye(25, [25, 41.4], 'surface'),
  ...openEye(39, [39, 41.4], 'surface'),
  stroked('M21 34 L28 32.5 M43 34 L36 32.5', 'surface', 1.8),
  stroked('M28.5 50.5 Q32 47.5 35.5 50.5', 'surface', 2),
]

const PAUSADO_SHAPES: MascotShape[] = [
  groundShadow(60, 17),
  {
    tag: 'path',
    attrs: {
      d: 'M14 52 C 14 42, 22 36, 32 36 C 42 36, 50 42, 50 52 C 50 57, 42 60, 32 60 C 22 60, 14 57, 14 52 Z',
    },
    fill: 'ink',
    part: 'brasa',
  },
  {
    tag: 'path',
    attrs: { d: 'M30 36 C 30 32, 33 30, 34 27 C 36 30, 35 34, 34 36 Z', opacity: 0.7 },
    fill: 'ink',
    part: 'brasa',
  },
  stroked('M21.5 48 Q25 51 28.5 48 M35.5 48 Q39 51 42.5 48', 'surface', 2),
  stroked('M29.5 54 Q32 55.6 34.5 54', 'surface', 1.8),
]

function headShapes(paint: MascotPaint, face: MascotPaint): MascotShape[] {
  return [
    ...flame(paint),
    ...CHEEKS,
    ...openEye(25, [25.6, 40.4], face, [26.5, 38.6]),
    ...openEye(39, [39.6, 40.4], face, [40.5, 38.6]),
    smile(face),
  ]
}

export const MASCOT_ART: Record<MascotLook, MascotArt> = {
  feliz: body(FELIZ_SHAPES),
  piscando: body([
    groundShadow(76, 20),
    ...LEGS,
    ...ARMS_DOWN,
    ...flame('ink'),
    ...CHEEKS,
    ...openEye(25, [25.8, 40.4], 'surface'),
    wink('surface'),
    smile('surface'),
  ]),
  animado: body(raisedArmsShapes()),
  'de-costas': body([
    groundShadow(76, 20),
    ...LEG_SHAPES,
    stroked('M13 44 C 8 46, 7 50, 8 54 M51 44 C 56 46, 57 50, 56 54', 'ink', 3.5),
    ...flame('ink'),
    {
      tag: 'path',
      attrs: { d: 'M24 30 C 26 36, 26 42, 24 48', 'stroke-width': 1.5, 'stroke-linecap': 'round', opacity: 0.25 },
      fill: 'none',
      stroke: '#fff',
    },
  ]),
  trabalhando: body(TRABALHANDO_SHAPES),
  'precisa-de-voce': body(PRECISA_DE_VOCE_SHAPES),
  'tudo-em-dia': body(FELIZ_SHAPES),
  erro: body(ERRO_SHAPES),
  pausado: body(PAUSADO_SHAPES),
  'atualizacao-pronta': body(raisedArmsShapes('bracos-cima')),
}

export const MASCOT_HEAD_ART: Record<MascotHead, MascotArt> = {
  cor: head(headShapes('brand', '#fff')),
  preto: head(headShapes('currentColor', 'surface')),
}
