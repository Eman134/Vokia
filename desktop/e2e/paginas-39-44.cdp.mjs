import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const CDP_PORT = process.env.PAGINAS_CDP_PORT ?? '9346'
const APP_URL = process.env.PAGINAS_APP_URL ?? 'http://localhost:4190/'
const DEBUG_LIST_URL = `http://127.0.0.1:${CDP_PORT}/json/list`
const OUT_DIR = process.argv[2] ?? 'e2e/evidencias-paginas-39-44'
const VIEWPORTS = [
  { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false },
  { width: 1200, height: 780, deviceScaleFactor: 1, mobile: false },
]
const SETTLE_MS = 300
const READY_TIMEOUT_MS = 15000
const READY_POLL_MS = 200
const SAMPLE_LABEL = 'dados de exemplo'
const MAX_COLUMN_WIDTH = 860
const MIN_CARD_WIDTH = 250
const MIN_SETTINGS_CONTROLS = 20
const OVERFLOW_TOLERANCE_PX = 1

const RAIL = { config: 'Configurações', canvas: 'Artefatos' }
const SETTINGS_TITLES = ['Geral', 'Agentes e conexões', 'Execução', 'Permissões', 'Barra de ação e avisos', 'Memória e grafo', 'Uso e limites', 'Atualizações', 'Privacidade e dados']
const SETTINGS_SUBTITLE = 'Valem para todos os projetos. Regras de um projeto ou de um agente ficam nas páginas deles e têm prioridade.'
const CONNECTION_TEXT = ['Tudo conectado', 'Claude Code, Codex, GitHub CLI e Git']
const THEME_LABELS = { light: 'Claro', dark: 'Escuro', system: 'Sistema' }
const ARTIFACTS_SUBTITLE = 'Quadros, diagramas, documentos e decisões do projeto. Você e os agentes criam e editam, e tudo fica salvo no repositório.'
const NEW_ARTIFACT = '+ Novo artefato'
const FILTER_LABELS = ['Todos 9', 'Whiteboard 1', 'Mermaid 2', 'PlantUML 1', 'Documento 2', 'Casos de uso 1', 'Anotações 1', 'Decisão (ADR) 1']
const CARDS = [
  ['Whiteboard', 'Arquitetura do checkout', 'Iara · agora'],
  ['Mermaid', 'Fluxo do pagamento Pix', 'Nina · 4 min'],
  ['PlantUML', 'Modelo de domínio de pagamentos', 'Iara · 1 h'],
  ['Documento', 'Spec · Pagamento via Pix', 'Téo · 20 min'],
  ['Casos de uso', 'Casos de uso do checkout', 'Rui · ontem'],
  ['Anotações', 'Anotações do checkout', 'Rui · 2 h'],
  ['Decisão (ADR)', 'ADR-007 · Efí como PSP de Pix', 'você · terça'],
  ['Mermaid', 'Estados do pedido', 'Davi · ontem'],
  ['Documento', 'Runbook · webhook fora do ar', 'Nina · 3 dias'],
]

const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
const results = []
const runtimeErrors = []
let send

function check(scenario, name, passed, detail = '') {
  results.push({ scenario, name, passed: Boolean(passed), detail })
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
  return {
    docScrollWidth: doc.scrollWidth,
    innerWidth: window.innerWidth,
    bodyScrollWidth: document.body.scrollWidth,
    pageScrollWidth: page?.scrollWidth ?? null,
    pageClientWidth: page?.clientWidth ?? null,
    pageFound: page !== null,
  }
}

async function checkNoHorizontalScroll(scenario) {
  const o = await pageRun(pageOverflow)
  check(
    scenario,
    'sem rolagem horizontal (documento e coluna rolável da página)',
    o.pageFound && o.docScrollWidth <= o.innerWidth + OVERFLOW_TOLERANCE_PX && o.bodyScrollWidth <= o.innerWidth + OVERFLOW_TOLERANCE_PX && o.pageScrollWidth <= o.pageClientWidth + OVERFLOW_TOLERANCE_PX,
    o,
  )
}

function pageSettingsHeader() {
  const h1 = document.querySelector('h1')
  const header = h1?.closest('header')
  const search = document.querySelector('input[type="search"]')
  const nav = document.querySelector('nav[aria-label="Seções"]')
  return {
    context: header?.querySelector('p')?.textContent.trim() ?? null,
    title: h1?.textContent.trim() ?? null,
    subtitle: h1?.nextElementSibling?.textContent.trim() ?? null,
    searchDisabled: search?.disabled ?? null,
    searchAria: search?.getAttribute('aria-disabled') ?? null,
    searchLabel: search?.getAttribute('aria-label') ?? null,
    sections: [...document.querySelectorAll('section[aria-label]')].map((item) => item.getAttribute('aria-label')),
    h2: [...document.querySelectorAll('section[aria-label] > header h2')].map((item) => item.textContent.trim()),
    navButtons: [...(nav?.querySelectorAll('button') ?? [])].map((item) => ({ text: item.textContent.trim(), disabled: item.disabled, current: item.getAttribute('aria-current') })),
    connection: [...(nav?.lastElementChild?.querySelectorAll(':scope > span') ?? [])].map((item) => item.textContent.trim()),
  }
}

function pageSettingsControls() {
  const root = document.querySelector('h1').closest('div[class*="overflow-y-auto"]')
  const controls = [...root.querySelectorAll('button, input, select, textarea')]
  const themeGroup = root.querySelector('[role="group"][aria-label="Tema"]')
  const describe = (item) => `${item.tagName.toLowerCase()}:${item.getAttribute('aria-label') ?? item.textContent.trim().slice(0, 30)}`
  const outside = controls.filter((item) => !themeGroup?.contains(item))
  return {
    outsideCount: outside.length,
    outsideEnabled: outside.filter((item) => !item.disabled).map(describe),
    themeEnabled: [...(themeGroup?.querySelectorAll('button') ?? [])].filter((item) => !item.disabled).length,
    themeButtons: [...(themeGroup?.querySelectorAll('button') ?? [])].map((item) => item.textContent.trim()),
    themeGroupDisabled: themeGroup?.getAttribute('aria-disabled') ?? null,
    switches: [...root.querySelectorAll('[role="switch"]')].map((item) => item.getAttribute('aria-checked')),
    selects: [...root.querySelectorAll('select')].map((item) => item.value),
    densityPressed: [...root.querySelectorAll('[role="group"][aria-label="Densidade"] button')].map((item) => `${item.textContent.trim()}=${item.getAttribute('aria-pressed')}`),
  }
}

function pageTheme() {
  return {
    attr: document.documentElement.getAttribute('data-theme'),
    bg: getComputedStyle(document.body).backgroundColor,
    pressed: [...document.querySelectorAll('[role="group"][aria-label="Tema"] button[aria-pressed="true"]')].map((item) => item.textContent.trim()),
  }
}

function pageSettingsLayout() {
  const root = document.querySelector('h1').closest('div[class*="overflow-y-auto"]')
  const sections = [...root.querySelectorAll('section[aria-label]')]
  const widths = sections.map((item) => Math.round(item.getBoundingClientRect().width))
  const overflowing = sections.filter((item) => item.scrollWidth > item.clientWidth + 1).map((item) => item.getAttribute('aria-label'))
  root.scrollTop = root.scrollHeight
  const last = sections[sections.length - 1].getBoundingClientRect()
  const form = document.querySelector('input[role="combobox"]').closest('form').getBoundingClientRect()
  const result = { maxSectionWidth: Math.max(...widths), overflowing, lastBottom: Math.round(last.bottom), barTop: Math.round(form.top), atBottom: Math.abs(root.scrollTop + root.clientHeight - root.scrollHeight) <= 1 }
  root.scrollTop = 0
  return result
}

function pageScrollSnapshot() {
  const root = document.querySelector('h1').closest('div[class*="overflow-y-auto"]')
  return { scrollTop: root.scrollTop, current: document.querySelector('nav[aria-label="Seções"] [aria-current]')?.textContent.trim() ?? null }
}

async function scenarioSettings(viewport) {
  const scenario = `Configurações ${viewport.width}x${viewport.height}`
  await reloadApp()
  await goToPage(RAIL.config)
  const h = await pageRun(pageSettingsHeader)
  check(scenario, 'contexto "Workspace · dados de exemplo", título, subtítulo e busca desabilitada', h.context === `Workspace · ${SAMPLE_LABEL}` && h.title === 'Configurações' && h.subtitle === SETTINGS_SUBTITLE && h.searchDisabled === true && h.searchAria === 'true' && h.searchLabel === 'Buscar configuração', h)
  check(scenario, 'as nove seções aparecem na ordem do protótipo', JSON.stringify(h.sections) === JSON.stringify(SETTINGS_TITLES) && JSON.stringify(h.h2) === JSON.stringify(SETTINGS_TITLES), { sections: h.sections })
  check(scenario, 'navegação lateral: nove entradas, todas desabilitadas, só Geral ativa', h.navButtons.length === 9 && h.navButtons.every((item) => item.disabled) && h.navButtons.filter((item) => item.current === 'page').map((item) => item.text).join() === 'Geral' && JSON.stringify(h.navButtons.map((item) => item.text)) === JSON.stringify(SETTINGS_TITLES), h.navButtons)
  check(scenario, 'indicador "Tudo conectado" com "Claude Code, Codex, GitHub CLI e Git" no fim da navegação', JSON.stringify(h.connection) === JSON.stringify(CONNECTION_TEXT), h.connection)

  const c = await pageRun(pageSettingsControls)
  check(scenario, 'controle positivo: o grupo Tema tem 3 botões habilitados (o detector enxerga controle ativo)', c.themeEnabled === 3 && c.themeButtons.length === 3 && c.themeGroupDisabled !== 'true', c.themeButtons)
  check(scenario, `todo controle fora do Tema está desabilitado (${c.outsideCount} controles)`, c.outsideCount > MIN_SETTINGS_CONTROLS && c.outsideEnabled.length === 0, c.outsideEnabled.slice(0, 5))

  // Cliques reais do mouse em controles desabilitados não mudam estado.
  const before = await pageRun(pageSettingsControls)
  await clickBy('[role="switch"]', null, 0)
  await clickBy('[role="group"][aria-label="Densidade"] button', 'Compacta')
  await clickBy('nav[aria-label="Seções"] button', 'Execução')
  const scrollAfterNav = await pageRun(pageScrollSnapshot)
  const after = await pageRun(pageSettingsControls)
  check(scenario, 'clicar em alternador, densidade e navegação lateral desabilitados não muda nada', JSON.stringify(before.switches) === JSON.stringify(after.switches) && JSON.stringify(before.densityPressed) === JSON.stringify(after.densityPressed) && JSON.stringify(before.selects) === JSON.stringify(after.selects) && scrollAfterNav.current === 'Geral', { before: before.densityPressed, after: after.densityPressed, scrollAfterNav })

  const layout = await pageRun(pageSettingsLayout)
  check(scenario, `seções não estouram a largura (máx ${MAX_COLUMN_WIDTH}) nem têm conteúdo cortado`, layout.maxSectionWidth <= MAX_COLUMN_WIDTH && layout.overflowing.length === 0, layout)
  check(scenario, 'ao rolar até o fim, a última seção termina acima da barra de ação', layout.atBottom && layout.lastBottom <= layout.barTop, layout)
  await checkNoHorizontalScroll(scenario)
  await screenshot(`configuracoes-${viewport.width}`)
}

async function scenarioTheme() {
  const scenario = 'Configurações · tema'
  await reloadApp()
  await goToPage(RAIL.config)
  const initial = await pageRun(pageTheme)
  check(scenario, 'estado inicial: Sistema marcado e sem data-theme', initial.attr === null && initial.pressed.join() === THEME_LABELS.system, initial)
  const backgrounds = {}
  for (const choice of ['dark', 'light', 'system']) {
    await clickBy('[role="group"][aria-label="Tema"] button', THEME_LABELS[choice])
    const now = await pageRun(pageTheme)
    backgrounds[choice] = now.bg
    check(scenario, `escolher ${THEME_LABELS[choice]} aplica o tema e marca só essa opção`, (choice === 'system' ? now.attr === null : now.attr === choice) && now.pressed.join() === THEME_LABELS[choice], now)
  }
  check(scenario, 'o fundo do app muda de fato entre Escuro e Claro', backgrounds.dark !== backgrounds.light, backgrounds)
  await clickBy('[role="group"][aria-label="Tema"] button', THEME_LABELS.dark)
  await goToPage(RAIL.canvas)
  const away = await pageRun(pageTheme)
  await goToPage(RAIL.config)
  const back = await pageRun(pageTheme)
  check(scenario, 'ao sair e reabrir Configurações, o tema escolhido segue aplicado e refletido no seletor', away.attr === 'dark' && back.attr === 'dark' && back.pressed.join() === THEME_LABELS.dark, { away, back })
  await screenshot('configuracoes-tema-escuro')
}

function pageArtifacts() {
  const h1 = document.querySelector('h1')
  const root = h1.closest('section')
  const cards = [...root.querySelectorAll('article')]
  const grid = cards[0]?.parentElement
  const filter = root.querySelector('[role="group"][aria-label="Tipo de artefato"]')
  const newButton = [...root.querySelectorAll('button')].find((item) => item.textContent.trim() === '+ Novo artefato')
  const rects = cards.map((item) => item.getBoundingClientRect())
  const gridBox = grid?.getBoundingClientRect()
  const overlaps = []
  rects.forEach((a, i) => rects.forEach((b, j) => {
    if (j > i && a.left < b.right - 1 && b.left < a.right - 1 && a.top < b.bottom - 1 && b.top < a.bottom - 1) overlaps.push([i, j])
  }))
  return {
    context: h1.parentElement.querySelector('p')?.textContent.trim() ?? null,
    title: h1.textContent.trim(),
    subtitle: h1.nextElementSibling?.textContent.trim() ?? null,
    newLabel: newButton?.textContent.trim() ?? null,
    newDisabled: newButton?.disabled ?? null,
    newAria: newButton?.getAttribute('aria-disabled') ?? null,
    filterDisabled: filter?.getAttribute('aria-disabled') ?? null,
    filters: [...(filter?.querySelectorAll('button') ?? [])].map((item) => ({ text: item.textContent.trim(), disabled: item.disabled, pressed: item.getAttribute('aria-pressed') })),
    countText: [...root.querySelectorAll('span')].map((item) => item.textContent.trim()).find((text) => /^\d+ artefatos$/.test(text)) ?? null,
    cards: cards.map((item) => [item.querySelector('p')?.textContent.trim(), item.querySelector('h3')?.textContent.trim(), item.querySelectorAll('p')[1]?.textContent.replace(/\s+/g, ' ').trim()]),
    cardsWithIcon: cards.filter((item) => item.querySelector('p svg path')).length,
    newArtifactDialog: document.body.textContent.includes('Que tipo de artefato?'),
    columns: new Set(rects.map((box) => Math.round(box.left))).size,
    cardsInsideGrid: gridBox ? rects.every((box) => box.left >= gridBox.left - 1 && box.right <= gridBox.right + 1) : false,
    overlaps,
    minCardWidth: Math.round(Math.min(...rects.map((box) => box.width))),
  }
}

function pageSnapshot() {
  const section = document.querySelector('h1')?.closest('section')
  return {
    h1: document.querySelector('h1')?.textContent.trim() ?? null,
    htmlHash: [...(section?.outerHTML ?? '')].reduce((hash, ch) => (hash * 31 + ch.charCodeAt(0)) >>> 0, 7),
    active: document.querySelector('button[aria-current="page"]')?.getAttribute('aria-label') ?? null,
    href: location.href,
    articles: document.querySelectorAll('article').length,
    dialogs: document.querySelectorAll('[role="dialog"], dialog').length,
  }
}

async function scenarioArtifacts(viewport) {
  const scenario = `Artefatos ${viewport.width}x${viewport.height}`
  await reloadApp()
  await goToPage(RAIL.canvas)
  const a = await pageRun(pageArtifacts)
  check(scenario, 'cabeçalho: contexto "loja-online · dados de exemplo", título e subtítulo', a.context === `loja-online · ${SAMPLE_LABEL}` && a.title === 'Artefatos' && a.subtitle === ARTIFACTS_SUBTITLE, a)
  check(scenario, 'botão "+ Novo artefato" visível e desabilitado', a.newLabel === NEW_ARTIFACT && a.newDisabled === true && a.newAria === 'true', a)
  check(scenario, 'seletor de tipo: Todos 9 ativo e demais com a contagem do protótipo, todos desabilitados', JSON.stringify(a.filters.map((item) => item.text)) === JSON.stringify(FILTER_LABELS) && a.filters.every((item) => item.disabled) && a.filters.filter((item) => item.pressed === 'true').map((item) => item.text).join() === 'Todos 9' && a.filterDisabled === 'true', a.filters)
  check(scenario, 'texto de contagem "9 artefatos"', a.countText === '9 artefatos', a.countText)
  check(scenario, 'há 9 cartões e a contagem de Todos é igual ao número de cartões', a.cards.length === 9 && a.filters[0]?.text === `Todos ${a.cards.length}`, a.cards.length)
  check(scenario, 'cada cartão mostra tipo (com ícone), título e autor · tempo do protótipo', JSON.stringify(a.cards) === JSON.stringify(CARDS) && a.cardsWithIcon === 9, a.cards)
  check(scenario, 'o seletor "Que tipo de artefato?" não faz parte do estado inicial', a.newArtifactDialog === false)
  check(scenario, 'a soma das contagens por tipo é igual a Todos', a.filters.slice(1).reduce((sum, item) => sum + Number(item.text.split(' ').pop()), 0) === 9)
  check(scenario, `grade: cartões dentro da grade, sem sobreposição e com largura mínima de ${MIN_CARD_WIDTH} px`, a.cardsInsideGrid && a.overlaps.length === 0 && a.minCardWidth >= MIN_CARD_WIDTH, { columns: a.columns, overlaps: a.overlaps, minCardWidth: a.minCardWidth })
  await checkNoHorizontalScroll(scenario)
  await screenshot(`artefatos-${viewport.width}`)

  const errorsBefore = runtimeErrors.length
  const snapBefore = await pageRun(pageSnapshot)
  const clicks = []
  for (let index = 0; index < CARDS.length; index += 1) {
    await clickBy('article', null, index)
    clicks.push(await pageRun(pageSnapshot))
  }
  check(scenario, 'clicar em cada um dos 9 cartões não muda a página (mesmo HTML, URL, item ativo do trilho, sem diálogo)', clicks.every((snap) => JSON.stringify(snap) === JSON.stringify(snapBefore)), { before: snapBefore, diverging: clicks.filter((snap) => JSON.stringify(snap) !== JSON.stringify(snapBefore)).slice(0, 2) })
  check(scenario, 'clicar nos cartões não gera erro de console nem exceção', runtimeErrors.length === errorsBefore, runtimeErrors.slice(errorsBefore, errorsBefore + 3))
  await clickBy('button', NEW_ARTIFACT)
  await clickBy('[role="group"][aria-label="Tipo de artefato"] button', 'Mermaid 2')
  const snapAfterDisabled = await pageRun(pageSnapshot)
  check(scenario, 'clicar em "+ Novo artefato" e num filtro desabilitado não muda a página', JSON.stringify(snapAfterDisabled) === JSON.stringify(snapBefore), snapAfterDisabled)
}

async function screenshot(name) {
  const { data } = await send('Page.captureScreenshot', { format: 'png' })
  writeFileSync(join(OUT_DIR, `${name}.png`), Buffer.from(data, 'base64'))
}

async function runScenario(scenario, fn) {
  try {
    await fn()
  } catch (error) {
    check(scenario, 'cenário roda até o fim', false, error.message)
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
      await runScenario(`Configurações ${viewport.width}x${viewport.height}`, () => scenarioSettings(viewport))
      await runScenario(`Artefatos ${viewport.width}x${viewport.height}`, () => scenarioArtifacts(viewport))
    }
    await runScenario('Configurações · tema', scenarioTheme)
    check('Console', 'nenhuma exceção nem console.error durante todo o roteiro', runtimeErrors.length === 0, runtimeErrors.slice(0, 5))
  } finally {
    await send('Emulation.clearDeviceMetricsOverride').catch(() => {})
    socket.close()
  }
  writeFileSync(join(OUT_DIR, 'resultados.json'), JSON.stringify({ checks: results, erros: runtimeErrors }, null, 2))
  for (const result of results) {
    console.log(`${result.passed ? 'PASSOU' : 'FALHOU'} | ${result.scenario} | ${result.name} | ${JSON.stringify(result.detail)}`)
  }
  const failed = results.filter((result) => !result.passed).length
  console.log(`RESUMO | ${results.length - failed} de ${results.length} PASSOU`)
  process.exitCode = failed === 0 ? 0 : 1
}

main().catch((error) => {
  console.error(error)
  process.exitCode = 1
})
