import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const CDP_PORT = process.env.SKILLS_CDP_PORT ?? '9348'
const APP_URL = process.env.SKILLS_APP_URL ?? 'http://localhost:4192/'
const DEBUG_LIST_URL = `http://127.0.0.1:${CDP_PORT}/json/list`
const OUT_DIR = process.argv[2] ?? 'e2e/evidencias-skills-38-memoria-42'
const REPORT_DIR = process.argv[3] ?? 'e2e'
const VIEWPORTS = [
  { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false },
  { width: 1200, height: 780, deviceScaleFactor: 1, mobile: false },
]
const NARROW_VIEWPORT = { width: 820, height: 700, deviceScaleFactor: 1, mobile: false }
const SETTLE_MS = 300
const READY_TIMEOUT_MS = 15000
const READY_POLL_MS = 200
const OVERFLOW_TOLERANCE_PX = 1
const SAMPLE_LABEL = 'dados de exemplo'
const RAIL = { skills: 'Skills', memoria: 'Memória e grafo' }
const SKILLS_STEP = 'skills-38'
const MEMORIA_STEP = 'memoria-42'

const SKILLS_CONTEXT = `Workspace · 8 skills · ${SAMPLE_LABEL}`
const SKILLS_SUBTITLE = 'Manuais que o agente segue numa situação. Você liga cada skill às ocupações que precisam dela, e o agente carrega só quando a tarefa pede.'
const SKILLS_ACTIONS = ['Criar da memória', 'Instalar skill']
const SKILLS_CARDS = [
  { name: 'Superpowers', origin: 'obra/superpowers', description: 'Pensar antes de codar, planejar em etapas, testar primeiro e provar que terminou. Alimenta a tela Plano.', chips: ['brainstorm', 'write-plan', 'execute-plan', 'test-driven-development', 'systematic-debugging', 'verification-before-completion'], bound: null },
  { name: 'shadcn/ui', origin: 'shadcn/skills', description: 'Contexto certo dos componentes, dos primitivos Radix e Base UI e do registry do projeto.', chips: [], bound: 'Ligada para Frontend e Designer' },
  { name: 'pix-loja-online', origin: null, description: 'Gerada de 6 memórias do projeto: txid, expiração do QR, sandbox do PSP e confirmação.', chips: [], bound: 'Ligada para Backend, Integrações e QA' },
]
const MATRIX_HEAD = ['Skill', 'Tech Lead', 'Frontend', 'Backend', 'Integrações', 'QA', 'Pesquisa']
const MATRIX_PORTRAITS = ['Iara', 'Davi', 'Nina', 'Léo', 'Téo', 'Rui']
const MATRIX_ROWS = [
  ['brainstorm', '1,0,0,0,0,1'],
  ['write-plan / execute-plan', '1,0,0,0,0,0'],
  ['test-driven-development', '0,1,1,1,1,0'],
  ['systematic-debugging', '0,1,1,1,1,0'],
  ['verification-before-completion', '1,1,1,1,1,1'],
  ['shadcn/ui', '0,1,0,0,0,0'],
  ['pix-loja-online', '0,0,1,1,1,0'],
]

const MEMORIA_CONTEXT = `loja-online · ${SAMPLE_LABEL}`
const MEMORIA_SUBTITLE = 'O que os agentes aprenderam sobre o projeto e o mapa que liga código e artefatos (diagramas, documentos e decisões) para ler menos e trabalhar sem se atropelar.'
const MEMORIA_FILTERS = ['Projeto', 'Por agente', 'Decisões', 'De artefatos', 'De devoluções', 'Suas preferências']
const MEMORIA_FACTS = [
  ['O webhook do PSP manda o txid em maiúsculas; o front normaliza.', 'Léo · da conversa Davi e Nina · usado em 3 tarefas'],
  ['Roteiros que buscam pedido por um id do PSP já incluem o passo de normalizar o id e um exemplo do payload real.', 'Iara · da devolução de PIX-4, que o Haiku resolveu com o roteiro v2 · aplicado em PAY-14.2'],
  ['QR Code do Pix expira em 15 minutos.', 'Iara · decisão aprovada por você · usado em 2 tarefas · ADR-004'],
  ['Nunca instalar dependência sem pedir aprovação.', 'sua preferência · vale para todos os agentes'],
  ['Testes de pagamento rodam com PSP_SANDBOX=1; sem isso a suíte chama a API real.', 'Nina · aprendido depois de um teste falhar · usado em 5 tarefas'],
  ['O pedido Pix passa por pendente → pago → expirado; só pago libera a nota.', 'Davi · extraído do diagrama · usado em 4 tarefas · Fluxo do pagamento Pix'],
  ['O checkout precisa funcionar com scripts de terceiros bloqueados.', 'Téo sugeriu · espera você confirmar'],
]
const MEMORIA_NOTE = 'Salva em .vokia/memory/ como Markdown, versionada com o código. Cada fato guarda a origem: conversa, tarefa ou artefato.'
const GRAPH_DESCRIPTION = 'arquivos do checkout, quem está mexendo em cada um e os artefatos que os descrevem'
const GRAPH_EMPTY_TITLE = 'Nenhum nó no grafo ainda'
const MEMORY_CARD_LABEL = 'Memória · fato com autor e uso'

const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
const results = []
const runtimeErrors = []
let send

function check(step, scenario, name, passed, detail = "") {
  results.push({ step, scenario, name, passed: Boolean(passed), detail })
}

async function connect() {
  const targets = await (await fetch(DEBUG_LIST_URL)).json()
  const target = targets.find((item) => item.type === 'page')
  if (!target) throw new Error(`nenhuma página no CDP ${DEBUG_LIST_URL}`)
  const socket = new WebSocket(target.webSocketDebuggerUrl)
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true })
    socket.addEventListener('error', () => reject(new Error('falha ao conectar no CDP')), { once: true })
  })
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data)
    if (message.method === 'Runtime.exceptionThrown') {
      runtimeErrors.push(message.params.exceptionDetails.exception?.description ?? message.params.exceptionDetails.text)
    }
    if (message.method === 'Runtime.consoleAPICalled' && message.params.type === 'error') {
      runtimeErrors.push(`console.error: ${message.params.args.map((arg) => arg.value ?? arg.description).join(' ')}`)
    }
  })
  return socket
}

function createClient(socket) {
  let nextId = 1
  const pending = new Map()
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data)
    const waiter = pending.get(message.id)
    if (waiter === undefined) return
    pending.delete(message.id)
    if (message.error) waiter.reject(new Error(message.error.message))
    else waiter.resolve(message.result)
  })
  return (method, params = {}) =>
    new Promise((resolve, reject) => {
      const id = nextId
      nextId += 1
      pending.set(id, { resolve, reject })
      socket.send(JSON.stringify({ id, method, params }))
    })
}

async function evaluate(expression) {
  const { result, exceptionDetails } = await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })
  if (exceptionDetails) throw new Error(exceptionDetails.exception?.description ?? exceptionDetails.text)
  return result.value
}

// Cada função de página é serializada e roda dentro do WebView: não pode usar constantes do Node.
const pageRun = (fn, ...args) => evaluate(`(${fn.toString()})(...${JSON.stringify(args)})`)

async function mouseClick(x, y) {
  const mouse = { x, y, button: 'left', clickCount: 1 }
  await send('Input.dispatchMouseEvent', { ...mouse, type: 'mouseMoved' })
  await send('Input.dispatchMouseEvent', { ...mouse, type: 'mousePressed' })
  await send('Input.dispatchMouseEvent', { ...mouse, type: 'mouseReleased' })
  await wait(SETTLE_MS)
}

function pageCenter(selector, text, index) {
  const matches = [...document.querySelectorAll(selector)].filter((item) => text === null || item.textContent.trim() === text || item.getAttribute('aria-label') === text)
  const element = matches[index]
  if (element === undefined) return null
  element.scrollIntoView({ block: 'center' })
  const box = element.getBoundingClientRect()
  return { x: box.left + box.width / 2, y: box.top + box.height / 2 }
}

async function clickBy(selector, text = null, index = 0) {
  const center = await pageRun(pageCenter, selector, text, index)
  if (center === null) throw new Error(`elemento não encontrado: ${selector} "${text}"`)
  await mouseClick(center.x, center.y)
}

const goToPage = (label) => clickBy('button[aria-label]', label)

function pageReady() {
  return Boolean(document.querySelector('#app')?.__vue_app__) && document.querySelector('nav[aria-label="Workspace"]') !== null && document.querySelector('input[role="combobox"]') !== null
}

async function reloadApp() {
  await send('Page.navigate', { url: APP_URL })
  await wait(800)
  const deadline = Date.now() + READY_TIMEOUT_MS
  while (!(await pageRun(pageReady))) {
    if (Date.now() > deadline) throw new Error('app não ficou pronto')
    await wait(READY_POLL_MS)
  }
}


function pageOverflow() {
  const doc = document.documentElement
  let page = document.querySelector('h1')?.parentElement ?? null
  while (page && getComputedStyle(page).overflowY !== 'auto') page = page.parentElement
  return { docScrollWidth: doc.scrollWidth, innerWidth: window.innerWidth, bodyScrollWidth: document.body.scrollWidth, pageScrollWidth: page?.scrollWidth ?? null, pageClientWidth: page?.clientWidth ?? null, pageFound: page !== null }
}

async function checkNoHorizontalScroll(step, scenario) {
  const o = await pageRun(pageOverflow)
  check(step, scenario, 'sem rolagem horizontal no documento nem na coluna rolável da página', o.pageFound && o.docScrollWidth <= o.innerWidth + OVERFLOW_TOLERANCE_PX && o.bodyScrollWidth <= o.innerWidth + OVERFLOW_TOLERANCE_PX && o.pageScrollWidth <= o.pageClientWidth + OVERFLOW_TOLERANCE_PX, o)
}

async function screenshot(name) {
  const { data } = await send('Page.captureScreenshot', { format: 'png' })
  writeFileSync(join(OUT_DIR, `${name}.png`), Buffer.from(data, 'base64'))
}

const same = (a, b) => JSON.stringify(a) === JSON.stringify(b)

// ---------------- Skills (Etapa 38) ----------------

function pageSkills() {
  const h1 = document.querySelector('h1')
  const root = h1.closest('section')
  const actions = [...root.querySelectorAll(':scope > div:first-child button')]
  const cards = [...root.querySelectorAll('article')]
  const grid = cards[0]?.parentElement
  const gridBox = grid?.getBoundingClientRect()
  const rects = cards.map((card) => card.getBoundingClientRect())
  const overlaps = []
  rects.forEach((a, i) => rects.forEach((b, j) => {
    if (j > i && a.left < b.right - 1 && b.left < a.right - 1 && a.top < b.bottom - 1 && b.top < a.bottom - 1) overlaps.push([i, j])
  }))
  const matrix = root.querySelector('section[aria-label="Quem usa o quê"]')
  const wrapper = matrix?.querySelector('.overflow-x-auto')
  const table = matrix?.querySelector('table')
  const heads = [...(table?.querySelectorAll('thead th') ?? [])]
  const rows = [...(table?.querySelectorAll('tbody tr') ?? [])]
  const markers = [...(matrix?.querySelectorAll('tbody [role="img"]') ?? [])]
  const portraits = heads.map((th) => th.querySelector('svg[role="img"]'))
  const chipsOutside = []
  cards.forEach((card, index) => {
    const box = card.getBoundingClientRect()
    card.querySelectorAll('span, button').forEach((chip) => {
      const c = chip.getBoundingClientRect()
      if (c.width > 0 && (c.right > box.right + 1 || c.left < box.left - 1)) chipsOutside.push(`${index}:${chip.textContent.trim()}`)
    })
  })
  return {
    context: h1.parentElement.querySelector('span')?.textContent.trim() ?? null,
    title: h1.textContent.trim(),
    subtitle: h1.nextElementSibling?.textContent.trim() ?? null,
    actions: actions.map((b) => ({ text: b.textContent.trim(), disabled: b.disabled, aria: b.getAttribute('aria-disabled') })),
    cards: cards.map((card) => {
      const header = card.querySelector('header')
      const name = card.querySelector('h2')?.textContent.trim() ?? null
      const paragraphs = [...card.querySelectorAll(':scope > p')].map((p) => p.textContent.trim())
      const chipBox = card.querySelector(':scope > div')
      const chips = [...(chipBox?.children ?? [])].map((c) => c.textContent.trim())
      const originSpan = [...(header?.querySelectorAll(':scope > span') ?? [])].map((s) => s.textContent.trim())[0] ?? null
      const badge = [...(header?.children ?? [])].filter((c) => c.tagName !== 'H2' && c.tagName.toLowerCase() !== 'svg').map((c) => c.textContent.trim())
      return { name, description: paragraphs[0] ?? null, bound: paragraphs.find((p) => p.startsWith('Ligada para')) ?? null, chips, originSpan, badge }
    }),
    cardsInsideGrid: gridBox ? rects.every((box) => box.left >= gridBox.left - 1 && box.right <= gridBox.right + 1) : false,
    overlaps,
    cardOverflow: cards.filter((card) => card.scrollWidth > card.clientWidth + 1).length,
    chipsOutside,
    matrix: {
      found: matrix !== null,
      title: matrix?.querySelector('h2')?.textContent.trim() ?? null,
      head: heads.map((th) => th.textContent.replace(/\s+/g, ' ').trim()),
      portraitLabels: portraits.slice(1).map((svg) => svg?.getAttribute('aria-label') ?? null),
      portraitArt: portraits.slice(1).map((svg) => svg?.querySelectorAll('path, circle, ellipse, rect').length ?? 0),
      portraitSizes: portraits.slice(1).map((svg) => Math.round(svg?.getBoundingClientRect().width ?? 0)),
      rows: rows.map((tr) => [tr.querySelector('td')?.textContent.trim(), [...tr.querySelectorAll('[role="img"]')].map((m) => (m.getAttribute('aria-label') === 'Ligada' ? 1 : 0)).join(',')]),
      markerCount: markers.length,
      markersDisabled: markers.every((m) => m.getAttribute('aria-disabled') === 'true'),
      markersFilled: markers.filter((m) => getComputedStyle(m).backgroundColor !== 'rgba(0, 0, 0, 0)').length,
      markersOn: markers.filter((m) => m.getAttribute('aria-label') === 'Ligada').length,
      wrapperOverflowX: wrapper ? getComputedStyle(wrapper).overflowX : null,
      wrapperScrollWidth: wrapper?.scrollWidth ?? null,
      wrapperClientWidth: wrapper?.clientWidth ?? null,
      tableWidth: Math.round(table?.getBoundingClientRect().width ?? 0),
      sectionWidth: Math.round(matrix?.getBoundingClientRect().width ?? 0),
    },
  }
}

function pageSkillsSnapshot() {
  const section = document.querySelector('h1')?.closest('section')
  return { hash: [...(section?.outerHTML ?? '')].reduce((hash, ch) => (hash * 31 + ch.charCodeAt(0)) >>> 0, 7), active: document.querySelector('button[aria-current="page"]')?.getAttribute('aria-label') ?? null, href: location.href, dialogs: document.querySelectorAll('[role="dialog"], dialog').length }
}

function pageScrollMatrix() {
  const wrapper = document.querySelector('section[aria-label="Quem usa o quê"] .overflow-x-auto')
  wrapper.scrollLeft = wrapper.scrollWidth
  const reached = wrapper.scrollLeft
  const last = [...wrapper.querySelectorAll('thead th')].pop().getBoundingClientRect()
  const view = wrapper.getBoundingClientRect()
  const result = { reached, lastVisible: last.right <= view.right + 1 && last.left >= view.left - 1 }
  wrapper.scrollLeft = 0
  return result
}

async function scenarioSkills(viewport) {
  const scenario = `Skills ${viewport.width}x${viewport.height}`
  const step = SKILLS_STEP
  await reloadApp()
  await goToPage(RAIL.skills)
  const s = await pageRun(pageSkills)
  check(step, scenario, 'contexto "Workspace · 8 skills · dados de exemplo", título "Skills" e subtítulo do protótipo', s.context === SKILLS_CONTEXT && s.title === 'Skills' && s.subtitle === SKILLS_SUBTITLE, { context: s.context, title: s.title, subtitle: s.subtitle })
  check(step, scenario, 'ações "Criar da memória" e "Instalar skill" nessa ordem, ambas desabilitadas com aria-disabled', same(s.actions.map((a) => a.text), SKILLS_ACTIONS) && s.actions.every((a) => a.disabled === true && a.aria === 'true'), s.actions)
  check(step, scenario, 'três cartões na ordem Superpowers, shadcn/ui, pix-loja-online', same(s.cards.map((c) => c.name), SKILLS_CARDS.map((c) => c.name)), s.cards.map((c) => c.name))
  SKILLS_CARDS.forEach((expected, index) => {
    const card = s.cards[index]
    const got = { originSpan: card?.originSpan, badge: card?.badge, description: card?.description, chips: card?.chips, bound: card?.bound }
    const originOk = expected.origin !== null ? card?.originSpan === expected.origin : same(card?.badge, ['criada da memória'])
    check(step, scenario, `cartão ${expected.name}: origem, descrição, chips e frase "Ligada para" exatos`, card !== undefined && originOk && card.description === expected.description && same(card.chips, expected.chips) && card.bound === expected.bound, got)
  })
  check(step, scenario, 'cartões dentro da grade, sem sobreposição, sem conteúdo cortado e com chips dentro do cartão (forma de falha: chips quebrando o cartão)', s.cardsInsideGrid && s.overlaps.length === 0 && s.cardOverflow === 0 && s.chipsOutside.length === 0, { overlaps: s.overlaps, cardOverflow: s.cardOverflow, chipsOutside: s.chipsOutside })

  const m = s.matrix
  check(step, scenario, 'matriz "Quem usa o quê" presente com cabeçalho Skill + seis ocupações na ordem do protótipo', m.found && m.title === 'Quem usa o quê' && same(m.head, MATRIX_HEAD), { title: m.title, head: m.head })
  check(step, scenario, 'cada ocupação tem retrato com arte (forma de falha: retrato sem arte), tamanho 22 e rótulo do agente certo', m.portraitLabels.length === 6 && m.portraitLabels.every((label, i) => label?.startsWith(`Retrato de ${MATRIX_PORTRAITS[i]},`)) && m.portraitArt.every((count) => count >= 8) && m.portraitSizes.every((size) => size === 22), { labels: m.portraitLabels, art: m.portraitArt, sizes: m.portraitSizes })
  check(step, scenario, 'sete linhas com o nome da skill e o marcador ligado/desligado de cada célula iguais ao protótipo', same(m.rows, MATRIX_ROWS), m.rows)
  const expectedOn = MATRIX_ROWS.reduce((sum, row) => sum + row[1].split(',').filter((v) => v === '1').length, 0)
  check(step, scenario, `42 marcadores (7x6), ${expectedOn} ligados, todos aria-disabled="true"`, m.markerCount === 42 && m.markersOn === expectedOn && m.markersDisabled, { count: m.markerCount, on: m.markersOn, disabled: m.markersDisabled })
  check(step, scenario, 'marcadores ligados têm preenchimento e desligados não (distinção visual real)', m.markersFilled === expectedOn, { filled: m.markersFilled, expectedOn })
  check(step, scenario, 'painel da matriz tem overflow-x auto: tabela mais larga que o painel rola dentro dele em vez de estourar a página (forma de falha: matriz estourando sem rolagem)', m.wrapperOverflowX === 'auto' && m.wrapperScrollWidth >= m.tableWidth - OVERFLOW_TOLERANCE_PX, m)
  await checkNoHorizontalScroll(step, scenario)

  const before = await pageRun(pageSkillsSnapshot)
  await clickBy('button', SKILLS_ACTIONS[0])
  await clickBy('button', SKILLS_ACTIONS[1])
  for (let index = 0; index < 3; index += 1) await clickBy('section[aria-label="Quem usa o quê"] tbody [role="img"]', null, index * 7 + 1)
  await clickBy('article', null, 2)
  const after = await pageRun(pageSkillsSnapshot)
  check(step, scenario, 'clicar nas duas ações, em marcadores da matriz e num cartão não muda a página (mesmo HTML, URL, trilho, sem diálogo)', same(before, after), { before, after })
  await screenshot(`skills-${viewport.width}`)
}

async function scenarioSkillsNarrow() {
  const scenario = `Skills ${NARROW_VIEWPORT.width}x${NARROW_VIEWPORT.height} (estreito)`
  const step = SKILLS_STEP
  await reloadApp()
  await goToPage(RAIL.skills)
  const s = await pageRun(pageSkills)
  await checkNoHorizontalScroll(step, scenario)
  check(step, scenario, 'estreito: matriz com rolagem horizontal própria (conteúdo maior que o painel) e cartões sem estourar', s.matrix.wrapperScrollWidth > s.matrix.wrapperClientWidth && s.cardOverflow === 0 && s.chipsOutside.length === 0, { scrollWidth: s.matrix.wrapperScrollWidth, clientWidth: s.matrix.wrapperClientWidth, cardOverflow: s.cardOverflow })
  const scrolled = await pageRun(pageScrollMatrix)
  check(step, scenario, 'estreito: rolando a matriz até o fim a coluna Pesquisa fica visível', scrolled.reached > 0 && scrolled.lastVisible, scrolled)
  await screenshot('skills-estreito')
}

// ---------------- Memória (Etapa 42) ----------------

function pageMemoria() {
  const h1 = document.querySelector('h1')
  const root = h1.closest('main')
  const memory = root.querySelector('section[aria-label="Memória"]')
  const graph = root.querySelector('section[aria-label="Grafo do código"]')
  const filter = memory?.querySelector('[role="group"][aria-label="Filtrar memória"]')
  const items = [...(memory?.querySelectorAll('ol > li') ?? [])]
  const guardar = [...root.querySelectorAll('button')].filter((b) => b.textContent.trim() === 'Guardar')
  const sectionBoxes = [memory, graph].map((s) => s?.getBoundingClientRect())
  const graphHeader = graph?.querySelector(':scope > div:first-child')
  const graphEmpty = graph?.querySelector(':scope > div:nth-child(2)')
  return {
    context: h1.parentElement.querySelector('span')?.textContent.trim() ?? null,
    title: h1.textContent.trim(),
    subtitle: h1.nextElementSibling?.textContent.replace(/\s+/g, ' ').trim() ?? null,
    sectionHeadings: [...root.querySelectorAll('section[aria-label] h2')].map((h) => h.textContent.trim()),
    sectionLabels: [...root.querySelectorAll('section[aria-label]')].map((s) => s.getAttribute('aria-label')),
    filter: {
      found: filter !== null && filter !== undefined,
      disabledAttr: filter?.getAttribute('aria-disabled') ?? null,
      buttons: [...(filter?.querySelectorAll('button') ?? [])].map((b) => ({ text: b.textContent.trim(), disabled: b.disabled, pressed: b.getAttribute('aria-pressed') })),
    },
    facts: items.map((li) => {
      const article = li.querySelector('article')
      const marker = li.firstElementChild
      const tag = marker?.tagName.toLowerCase() ?? null
      return {
        label: article?.querySelector('p')?.textContent.trim() ?? null,
        text: article?.querySelectorAll('p')[1]?.textContent.trim() ?? null,
        footer: article?.querySelector('footer span')?.textContent.replace(/\s+/g, ' ').trim() ?? null,
        markerTag: tag,
        markerText: tag === 'span' ? marker.textContent.trim() : null,
        markerLabel: marker?.getAttribute('aria-label') ?? null,
        markerBg: marker ? getComputedStyle(marker).backgroundColor : null,
        articleOverflow: article ? article.scrollWidth > article.clientWidth + 1 : null,
        liInside: article ? article.getBoundingClientRect().right <= memory.getBoundingClientRect().right + 1 : null,
      }
    }),
    guardar: guardar.map((b) => ({ disabled: b.disabled, aria: b.getAttribute('aria-disabled'), inLastItem: items.length > 0 && items[items.length - 1].contains(b) })),
    note: [...(memory?.querySelectorAll('p') ?? [])].map((p) => p.textContent.trim()).find((t) => t.startsWith('Salva em')) ?? null,
    graph: {
      found: graph !== null && graph !== undefined,
      heading: graph?.querySelector('h2')?.textContent.trim() ?? null,
      description: graphHeader?.querySelector('p')?.textContent.trim() ?? null,
      emptyParagraphs: [...(graphEmpty?.querySelectorAll('p') ?? [])].map((p) => p.textContent.trim()),
      visibleText: (graph?.innerText ?? '').replace(/\s+/g, ' ').trim(),
      drawn: graph ? graph.querySelectorAll('svg, canvas, img, video, iframe, li, article, [role="img"]').length : null,
    },
    layout: {
      sameRow: sectionBoxes[0] && sectionBoxes[1] ? Math.abs(sectionBoxes[0].top - sectionBoxes[1].top) <= 1 : null,
      memoryWidth: Math.round(sectionBoxes[0]?.width ?? 0),
      graphWidth: Math.round(sectionBoxes[1]?.width ?? 0),
      stackedGap: sectionBoxes[0] && sectionBoxes[1] ? Math.round(sectionBoxes[1].top - sectionBoxes[0].bottom) : null,
      overlap: sectionBoxes[0] && sectionBoxes[1] ? sectionBoxes[0].left < sectionBoxes[1].right - 1 && sectionBoxes[1].left < sectionBoxes[0].right - 1 && sectionBoxes[0].top < sectionBoxes[1].bottom - 1 && sectionBoxes[1].top < sectionBoxes[0].bottom - 1 : null,
      memoryClip: memory ? memory.scrollWidth > memory.clientWidth + 1 : null,
      graphClip: graph ? graph.scrollWidth > graph.clientWidth + 1 : null,
    },
  }
}

function pageMemoriaSnapshot() {
  const main = document.querySelector('h1')?.closest('main')
  return { hash: [...(main?.outerHTML ?? '')].reduce((hash, ch) => (hash * 31 + ch.charCodeAt(0)) >>> 0, 7), items: main?.querySelectorAll('ol > li').length ?? 0, active: document.querySelector('button[aria-current="page"]')?.getAttribute('aria-label') ?? null, href: location.href, dialogs: document.querySelectorAll('[role="dialog"], dialog').length }
}

async function scenarioMemoria(viewport) {
  const scenario = `Memória ${viewport.width}x${viewport.height}`
  const step = MEMORIA_STEP
  await reloadApp()
  await goToPage(RAIL.memoria)
  const m = await pageRun(pageMemoria)
  check(step, scenario, 'contexto "loja-online · dados de exemplo", título "Memória e grafo" e subtítulo do protótipo', m.context === MEMORIA_CONTEXT && m.title === 'Memória e grafo' && m.subtitle === MEMORIA_SUBTITLE, { context: m.context, title: m.title, subtitle: m.subtitle })
  check(step, scenario, 'duas seções, Memória e Grafo, nessa ordem', same(m.sectionLabels, ['Memória', 'Grafo do código']) && same(m.sectionHeadings, ['Memória', 'Grafo']), { labels: m.sectionLabels, headings: m.sectionHeadings })
  check(step, scenario, 'filtro em cápsula "Filtrar memória" com as seis opções na ordem, todas desabilitadas, só Projeto ativo', m.filter.found && same(m.filter.buttons.map((b) => b.text), MEMORIA_FILTERS) && m.filter.buttons.every((b) => b.disabled) && m.filter.disabledAttr === 'true' && same(m.filter.buttons.filter((b) => b.pressed === 'true').map((b) => b.text), ['Projeto']), m.filter)
  check(step, scenario, 'sete fatos na ordem do protótipo, cada um em UiMemoryCard com rótulo, texto e "autor · uso" exatos', m.facts.length === 7 && m.facts.every((f, i) => f.label === MEMORY_CARD_LABEL && f.text === MEMORIA_FACTS[i][0] && f.footer === MEMORIA_FACTS[i][1]), m.facts.map((f) => [f.text, f.footer]))
  const pref = m.facts[3]
  check(step, scenario, 'preferência do usuário: marcador é a inicial "K" em círculo escuro (não retrato) e os outros seis têm retrato do agente', pref?.markerTag === 'span' && pref.markerText === 'K' && pref.markerBg !== 'rgba(0, 0, 0, 0)' && m.facts.filter((_, i) => i !== 3).every((f) => f.markerTag === 'svg' && f.markerLabel?.startsWith('Retrato de ')), m.facts.map((f) => [f.markerTag, f.markerText ?? f.markerLabel]))
  check(step, scenario, 'retratos na ordem dos autores: Léo, Iara, Iara, Nina, Davi, Téo', same(m.facts.filter((_, i) => i !== 3).map((f) => f.markerLabel?.split(',')[0]), ['Retrato de Léo', 'Retrato de Iara', 'Retrato de Iara', 'Retrato de Nina', 'Retrato de Davi', 'Retrato de Téo']), m.facts.map((f) => f.markerLabel))
  check(step, scenario, 'botão "Guardar": existe um só, no último fato (sugestão do Téo), desabilitado com aria-disabled', m.guardar.length === 1 && m.guardar[0].disabled === true && m.guardar[0].aria === 'true' && m.guardar[0].inLastItem, m.guardar)
  check(step, scenario, 'nota de rodapé "Salva em .vokia/memory/ ..." exata', m.note === MEMORIA_NOTE, m.note)
  check(step, scenario, 'fatos longos não estouram o cartão nem a seção (forma de falha: fato longo estourando)', m.facts.every((f) => f.articleOverflow === false && f.liInside === true), m.facts.map((f) => [f.articleOverflow, f.liInside]))
  check(step, scenario, 'Grafo: título "Grafo", descrição do protótipo e bloco Vazio só com o título "Nenhum nó no grafo ainda" (sem parágrafo em branco)', m.graph.found && m.graph.heading === 'Grafo' && m.graph.description === GRAPH_DESCRIPTION && same(m.graph.emptyParagraphs, [GRAPH_EMPTY_TITLE]), m.graph)
  check(step, scenario, 'Grafo: nenhum nó, imagem, canvas, lista ou cartão desenhado, e nenhum texto além de título, descrição e Vazio (forma de falha: grafo improvisado)', m.graph.drawn === 0 && m.graph.visibleText === `Grafo ${GRAPH_DESCRIPTION} ${GRAPH_EMPTY_TITLE}`, { drawn: m.graph.drawn, visibleText: m.graph.visibleText })
  if (viewport.width >= 1200) {
    check(step, scenario, 'seções lado a lado na mesma linha, sem sobreposição e sem conteúdo cortado (forma de falha: seções empilhando mal)', m.layout.sameRow === true && m.layout.overlap === false && m.layout.memoryClip === false && m.layout.graphClip === false, m.layout)
  }
  check(step, scenario, 'largura da seção Memória >= 400 (base do protótipo)', m.layout.memoryWidth >= 400, m.layout)
  await checkNoHorizontalScroll(step, scenario)

  const before = await pageRun(pageMemoriaSnapshot)
  await clickBy('[role="group"][aria-label="Filtrar memória"] button', 'Decisões')
  await clickBy('[role="group"][aria-label="Filtrar memória"] button', 'Suas preferências')
  await clickBy('button', 'Guardar')
  const afterDisabled = await pageRun(pageMemoriaSnapshot)
  check(step, scenario, 'clicar em dois filtros e em "Guardar" desabilitados não muda a página (mesmo HTML, URL, trilho, sem diálogo)', same(before, afterDisabled), { before, afterDisabled })
  const errorsBefore = runtimeErrors.length
  for (let index = 0; index < 7; index += 1) await clickBy('button', 'Esquecer', index)
  const afterForget = await pageRun(pageMemoriaSnapshot)
  check(step, scenario, 'clicar nos sete "Esquecer" não tem efeito: nenhum fato some, HTML igual, sem diálogo nem erro de console', same(before, afterForget) && afterForget.items === 7 && runtimeErrors.length === errorsBefore, { before, afterForget, errors: runtimeErrors.slice(errorsBefore, errorsBefore + 3) })
  await screenshot(`memoria-${viewport.width}`)
}

// Prova de utilidade sem tocar a produção: injeta no DOM os defeitos que os detectores devem pegar.
async function scenarioMutations() {
  const scenario = 'Mutação no DOM (detectores)'
  await reloadApp()
  await goToPage(RAIL.memoria)
  const clean = await pageRun(pageMemoria)
  await pageRun(() => {
    const root = document.querySelector('section[aria-label="Grafo do código"]')
    root.insertAdjacentHTML('beforeend', '<svg width="40" height="40"><circle cx="20" cy="20" r="10"/></svg>')
    const memory = document.querySelector('section[aria-label="Memória"]')
    memory.querySelector('[role="group"] button').disabled = false
    document.querySelectorAll('button').forEach((b) => { if (b.textContent.trim() === 'Guardar') b.disabled = false })
    memory.querySelector('ol > li:nth-child(4) > span').textContent = 'X'
    memory.querySelector('ol > li:nth-child(2) article > p:nth-child(2)').textContent = 'texto alterado'
  })
  const mutated = await pageRun(pageMemoria)
  check(MEMORIA_STEP, scenario, 'controle: página limpa tem 0 nós desenhados no Grafo e "Guardar" desabilitado', clean.graph.drawn === 0 && clean.guardar[0].disabled === true, { drawn: clean.graph.drawn, guardar: clean.guardar })
  check(MEMORIA_STEP, scenario, 'detector pega um nó de grafo improvisado (drawn > 0)', mutated.graph.drawn > 0, mutated.graph.drawn)
  check(MEMORIA_STEP, scenario, 'detector pega filtro e "Guardar" habilitados', mutated.filter.buttons.some((b) => !b.disabled) && mutated.guardar[0].disabled === false, { filter: mutated.filter.buttons[0], guardar: mutated.guardar[0] })
  check(MEMORIA_STEP, scenario, 'detector pega a inicial trocada e o texto de fato alterado', mutated.facts[3].markerText !== 'K' && mutated.facts[1].text !== MEMORIA_FACTS[1][0], { marker: mutated.facts[3].markerText, text: mutated.facts[1].text })

  await goToPage(RAIL.skills)
  await pageRun(() => {
    const matrix = document.querySelector('section[aria-label="Quem usa o quê"]')
    matrix.querySelector('.overflow-x-auto').style.overflowX = 'visible'
    matrix.querySelector('tbody [role="img"]').setAttribute('aria-label', 'Desligada')
    matrix.querySelector('thead th:nth-child(2) svg').innerHTML = ''
    document.querySelector('h1').closest('section').querySelector('button').disabled = false
  })
  const s = await pageRun(pageSkills)
  check(SKILLS_STEP, scenario, 'detector pega ação habilitada, célula alterada, retrato sem arte e matriz sem rolagem', s.actions[0].disabled === false && s.matrix.rows[0][1] !== MATRIX_ROWS[0][1] && s.matrix.portraitArt[0] === 0 && s.matrix.wrapperOverflowX !== 'auto', { action: s.actions[0], row: s.matrix.rows[0], art: s.matrix.portraitArt[0], overflowX: s.matrix.wrapperOverflowX })
}

const HISTORY_MARKER = '<!-- rodadas-anteriores -->'
const ROUND = process.env.SKILLS_RODADA ?? '2'

// Preserva as rodadas já registradas: o relatório da rodada atual vai no topo e o histórico fica abaixo do marcador.
function withHistory(path, report) {
  let previous = ''
  try {
    previous = readFileSync(path, 'utf8')
  } catch {
    return report
  }
  const index = previous.indexOf(HISTORY_MARKER)
  const history = index >= 0 ? previous.slice(index + HISTORY_MARKER.length) : `
## Rodada 1

${previous.replace(/^# /, '### ')}`
  return `${report}
${HISTORY_MARKER}${history}`
}

function renderReport(step, title, rows, note) {
  const mine = rows.filter((r) => r.step === step)
  const failures = mine.filter((r) => !r.passed)
  const lines = [
    `# RESULTADO - ${title} (rodada ${ROUND}, atual)`,
    '',
    `Veredito: **${failures.length === 0 ? 'PASSOU' : 'FALHOU'}** (${mine.length - failures.length} de ${mine.length} verificações)`,
    '',
    'Repetir: a partir de `desktop/`, `bash e2e/skills-memoria-rodar.sh` (Vite de teste na 4192 e Edge headless com CDP na 9348; só esses dois processos são parados no fim).',
    'Arquivo de teste: `desktop/e2e/skills-memoria.cdp.mjs`. Evidências (capturas, `resultados.json`, `run.log`): `desktop/e2e/evidencias-skills-38-memoria-42/`.',
    '',
    note,
    '',
    '| Resultado | Cenário | Verificação |',
    '|---|---|---|',
    ...mine.map((r) => `| ${r.passed ? 'PASSOU' : 'FALHOU'} | ${r.scenario} | ${r.name.replace(/\|/g, '\\|')} |`),
  ]
  if (failures.length > 0) {
    lines.push('', '## Falhas', '', ...failures.map((r) => `- ${r.scenario} · ${r.name}: \`${JSON.stringify(r.detail).slice(0, 600)}\``))
  }
  return `${lines.join('\n')}\n`
}

async function runScenario(step, scenario, fn) {
  try {
    await fn()
  } catch (error) {
    check(step, scenario, 'cenário roda até o fim', false, error.message)
  }
}

async function main() {
  mkdirSync(OUT_DIR, { recursive: true })
  const socket = await connect()
  send = createClient(socket)
  try {
    await send('Runtime.enable')
    await send('Page.enable')
    for (const viewport of VIEWPORTS) {
      await send('Emulation.setDeviceMetricsOverride', viewport)
      await runScenario(SKILLS_STEP, `Skills ${viewport.width}x${viewport.height}`, () => scenarioSkills(viewport))
      await runScenario(MEMORIA_STEP, `Memória ${viewport.width}x${viewport.height}`, () => scenarioMemoria(viewport))
    }
    await send('Emulation.setDeviceMetricsOverride', NARROW_VIEWPORT)
    await runScenario(SKILLS_STEP, 'Skills estreito', scenarioSkillsNarrow)
    await send('Emulation.setDeviceMetricsOverride', VIEWPORTS[0])
    await runScenario(MEMORIA_STEP, 'Mutação no DOM', scenarioMutations)
    check(SKILLS_STEP, 'Console', 'nenhuma exceção nem console.error durante todo o roteiro', runtimeErrors.length === 0, runtimeErrors.slice(0, 5))
    check(MEMORIA_STEP, 'Console', 'nenhuma exceção nem console.error durante todo o roteiro', runtimeErrors.length === 0, runtimeErrors.slice(0, 5))
  } finally {
    await send('Emulation.clearDeviceMetricsOverride').catch(() => {})
    socket.close()
  }
  writeFileSync(join(OUT_DIR, 'resultados.json'), JSON.stringify({ checks: results, erros: runtimeErrors }, null, 2))
  writeFileSync(join(REPORT_DIR, 'RESULTADO-skills-38.md'), renderReport(SKILLS_STEP, 'Etapa 38 · Página Skills', results, 'Cobre cabeçalho, três cartões, matriz "Quem usa o quê", ações e marcadores desabilitados, rolagem da matriz a 820 px e leiaute a 1200 e 1440 px. Os detectores foram provados em "Mutação no DOM" (a produção não é alterada).'))
  writeFileSync(join(REPORT_DIR, 'RESULTADO-memoria-42.md'), withHistory(join(REPORT_DIR, 'RESULTADO-memoria-42.md'), renderReport(MEMORIA_STEP, 'Etapa 42 · Página Memória e grafo', results, 'Cobre cabeçalho, seções Memória e Grafo, filtro desabilitado, sete fatos, "Guardar" desabilitado, "Esquecer" sem efeito, nota de rodapé e o Vazio do Grafo sem nó desenhado. Os detectores foram provados em "Mutação no DOM" (a produção não é alterada).')))
  for (const result of results) {
    console.log(`${result.passed ? 'PASSOU' : 'FALHOU'} | ${result.step} | ${result.scenario} | ${result.name} | ${JSON.stringify(result.detail)}`)
  }
  const failed = results.filter((result) => !result.passed).length
  console.log(`RESUMO | ${results.length - failed} de ${results.length} PASSOU`)
  process.exitCode = failed === 0 ? 0 : 1
}

main().catch((error) => {
  console.error(error)
  process.exitCode = 1
})
