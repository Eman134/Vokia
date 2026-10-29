import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const CDP_PORT = process.env.ONBOARDING_CDP_PORT ?? '9356'
const APP_URL = process.env.ONBOARDING_APP_URL ?? 'http://localhost:4196/'
const DEBUG_LIST_URL = `http://127.0.0.1:${CDP_PORT}/json/list`
const OUT_DIR = process.argv[2] ?? 'e2e/evidencias-onboarding'
const VIEWPORTS = [
  { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false },
  { width: 1200, height: 780, deviceScaleFactor: 1, mobile: false },
]
const SETTLE_MS = 300
const READY_TIMEOUT_MS = 15000
const READY_POLL_MS = 200
const SAMPLE_LABEL = 'dados de exemplo'
const COLUMN_WIDTH = 600
const TITLE_FONT_PX = 34
const TITLE_LINE_HEIGHT_PX = 37.4
const OVERFLOW_TOLERANCE_PX = 1
const MIN_CONTRAST = 4.5
const SKIP_LABEL = 'Pular e ir para o fim →'
const BACK_LABEL = 'Voltar'
const SEND_LABEL = 'Enviar para a Iara'
const NEW_PROJECT = 'Novo projeto'
const MAP_LABEL = 'Mapa do time'
const THEME_LABELS = { light: 'Claro', dark: 'Escuro', system: 'Sistema' }
const CONTRAST_KEYS = ['title', 'subtitle', 'forward', 'marker']

const STEPS = [
  { id: 'boas-vindas', title: 'Boas-vindas à Vokia', subtitle: 'Um time de agentes de IA trabalhando no seu código, no seu computador. São cinco passos rápidos.', forward: 'Começar', forwardDisabled: false, sample: false, skip: true },
  { id: 'conexoes', title: 'Conecte seus agentes', subtitle: 'A Vokia usa os CLIs que já estão no seu computador. O login é sempre o deles.', forward: 'Continuar só com Claude', forwardDisabled: false, sample: true, skip: true },
  { id: 'projeto', title: 'Escolha um projeto', subtitle: 'Uma pasta com git ou um repositório do GitHub. Cada feature ganha uma branch e um worktree próprios.', forward: 'Continuar', forwardDisabled: false, sample: true, skip: true },
  { id: 'time', title: 'Monte o time', subtitle: 'Comece de um modelo. Dá para trocar agentes, camadas e quem lidera depois, no Mapa do time.', forward: 'Continuar', forwardDisabled: false, sample: true, skip: true },
  { id: 'pedido', title: 'O que o time faz primeiro?', subtitle: 'Iara transforma o pedido em uma feature com cards e espera o seu ok antes de começar.', forward: SEND_LABEL, forwardDisabled: true, sample: true, skip: true },
  { id: 'pronto', title: 'Tudo pronto', subtitle: 'Iara já está lendo o loja-online. A primeira proposta chega na barra de baixo em uns 2 minutos.', forward: 'Abrir o mapa do time', forwardDisabled: false, sample: true, skip: false },
]
const LAST = STEPS.length - 1
const PEDIDO = 4

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

const clickButton = (label) => clickBy('button', label)
const clickTheme = (choice) => clickBy('[role="group"][aria-label="Tema"] button', THEME_LABELS[choice])

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

function pageAppState() {
  const shell = document.querySelector('div.h-screen')
  return {
    railPresent: document.querySelector('nav[aria-label="Workspace"]') !== null,
    railButtons: document.querySelectorAll('nav[aria-label="Workspace"] button[aria-label]').length,
    railWidthVar: shell?.style.getPropertyValue('--rail-w') ?? null,
    active: document.querySelector('button[aria-current="page"]')?.getAttribute('aria-label') ?? null,
    h1: document.querySelector('h1')?.textContent.trim() ?? null,
  }
}

// Mede a casca do onboarding: texto, rodapé, marcador, coluna e posição do botão final em relação à barra de ação.
function pageOnboarding() {
  const h1 = document.querySelector('h1')
  const root = h1.closest('div.overflow-y-auto')
  const main = root.querySelector('main')
  const buttons = [...main.querySelectorAll('footer button')]
  const skip = [...root.querySelectorAll('button')].find((item) => item.textContent.trim().startsWith('Pular'))
  const marker = [...main.querySelectorAll('p')].find((item) => item.textContent.trim() === 'dados de exemplo')
  const bar = document.querySelector('input[role="combobox"]').closest('div[class*="fixed"]')
  const rootBox = root.getBoundingClientRect()
  const mainBox = main.getBoundingClientRect()
  const titleStyle = getComputedStyle(h1)
  const forward = buttons[buttons.length - 1]
  const forwardBox = forward.getBoundingClientRect()
  const barBox = bar.getBoundingClientRect()
  const hit = document.elementFromPoint(forwardBox.left + forwardBox.width / 2, forwardBox.top + forwardBox.height / 2)
  return {
    h1: h1.textContent.trim(),
    subtitle: h1.nextElementSibling?.textContent.trim() ?? null,
    marker: marker !== undefined,
    markerBeforeTitle: marker !== undefined && Boolean(marker.compareDocumentPosition(h1) & Node.DOCUMENT_POSITION_FOLLOWING),
    brand: root.querySelector('header b')?.textContent.trim() ?? null,
    footer: buttons.map((item) => ({ text: item.textContent.trim(), disabled: item.disabled })),
    skip: skip ? { text: skip.textContent.trim() } : null,
    rootLeft: Math.round(rootBox.left),
    rootWidth: Math.round(rootBox.width),
    columnWidth: Math.round(mainBox.width),
    // clientWidth exclui a barra de rolagem vertical, que desloca o centro visível quando a tela é alta.
    columnCenterOffset: Math.round(mainBox.left + mainBox.width / 2 - (rootBox.left + root.clientWidth / 2)),
    fontSize: titleStyle.fontSize,
    lineHeight: titleStyle.lineHeight,
    forwardTop: Math.round(forwardBox.top),
    forwardBottom: Math.round(forwardBox.bottom),
    barTop: Math.round(barBox.top),
    forwardCovered: !(hit === forward || forward.contains(hit)),
    forwardHitTag: hit?.tagName ?? null,
    forwardOverlapsBar: forwardBox.bottom > barBox.top && forwardBox.top < barBox.bottom && forwardBox.left < barBox.right && forwardBox.right > barBox.left,
  }
}

function pageScrollToEnd() {
  const root = document.querySelector('h1').closest('div.overflow-y-auto')
  root.scrollTop = root.scrollHeight
}

function pageOverflow() {
  const page = document.querySelector('h1').closest('div.overflow-y-auto')
  return {
    docScrollWidth: document.documentElement.scrollWidth,
    innerWidth: window.innerWidth,
    bodyScrollWidth: document.body.scrollWidth,
    pageScrollWidth: page.scrollWidth,
    pageClientWidth: page.clientWidth,
  }
}

async function checkNoHorizontalScroll(scenario, label) {
  const o = await pageRun(pageOverflow)
  check(scenario, `${label}: sem rolagem horizontal`, o.docScrollWidth <= o.innerWidth + OVERFLOW_TOLERANCE_PX && o.bodyScrollWidth <= o.innerWidth + OVERFLOW_TOLERANCE_PX && o.pageScrollWidth <= o.pageClientWidth + OVERFLOW_TOLERANCE_PX, o)
}

// Cor via canvas: aceita rgb(), oklch(), color-mix() e qualquer sintaxe que o navegador entenda.
function pageContrast() {
  const canvas = document.createElement('canvas')
  canvas.width = 1
  canvas.height = 1
  const ctx = canvas.getContext('2d', { willReadFrequently: true })
  const toRgb = (color) => {
    ctx.clearRect(0, 0, 1, 1)
    ctx.fillStyle = '#000'
    ctx.fillStyle = color
    ctx.fillRect(0, 0, 1, 1)
    const [r, g, b, a] = ctx.getImageData(0, 0, 1, 1).data
    return { r, g, b, a: a / 255 }
  }
  const luminance = ({ r, g, b }) => {
    const channel = (v) => {
      const s = v / 255
      return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4
    }
    return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
  }
  const backgroundOf = (element) => {
    for (let node = element; node; node = node.parentElement) {
      const rgb = toRgb(getComputedStyle(node).backgroundColor)
      if (rgb.a > 0.99) return rgb
    }
    return toRgb(getComputedStyle(document.body).backgroundColor)
  }
  const ratio = (element) => {
    const [hi, lo] = [luminance(toRgb(getComputedStyle(element).color)), luminance(backgroundOf(element))].sort((a, b) => b - a)
    return Math.round(((hi + 0.05) / (lo + 0.05)) * 100) / 100
  }
  const h1 = document.querySelector('h1')
  const root = h1.closest('div.overflow-y-auto')
  const forward = [...root.querySelectorAll('footer button')].pop()
  const marker = [...root.querySelectorAll('main p')].find((item) => item.textContent.trim() === 'dados de exemplo')
  const result = { title: ratio(h1), subtitle: ratio(h1.nextElementSibling), forward: ratio(forward) }
  if (marker) result.marker = ratio(marker)
  return result
}

function pageDisabled() {
  const content = document.querySelector('h1').closest('main > div')
  const interactive = [...content.querySelectorAll('button, textarea, input, select')].filter((item) => item.closest('[role="group"][aria-label="Tema"]') === null)
  return {
    total: interactive.length,
    enabled: interactive.filter((item) => !item.disabled).map((item) => item.textContent.trim() || item.getAttribute('aria-label')),
    groups: [...content.querySelectorAll('[role="group"]')].filter((item) => item.getAttribute('aria-label') !== 'Tema').map((item) => ({ label: item.getAttribute('aria-label'), ariaDisabled: item.getAttribute('aria-disabled') })),
  }
}

function pageTheme() {
  const root = document.querySelector('h1').closest('div.overflow-y-auto')
  return {
    attr: document.documentElement.getAttribute('data-theme'),
    bg: getComputedStyle(root).backgroundColor,
    pressed: [...document.querySelectorAll('[role="group"][aria-label="Tema"] button[aria-pressed="true"]')].map((item) => item.textContent.trim()),
  }
}

function pageStepContent() {
  return document.querySelector('h1').closest('main > div').innerText.replace(/\s+/g, ' ').trim()
}

async function screenshot(name) {
  const { data } = await send('Page.captureScreenshot', { format: 'png' })
  writeFileSync(join(OUT_DIR, `${name}.png`), Buffer.from(data, 'base64'))
}

async function openOnboarding() {
  await reloadApp()
  await clickBy('button[aria-label]', NEW_PROJECT)
}

async function goToStep(target) {
  for (let i = 0; i < target; i += 1) await clickButton(STEPS[i].forward)
}

// Avança uma tela; em Pedido o botão de envio é desabilitado, então a saída é o "Pular".
async function advance(index) {
  if (index === PEDIDO) await clickBy('button', SKIP_LABEL)
  else await clickButton(STEPS[index].forward)
}

function checkDisabledControls(scenario, label, step, state) {
  if (step.id === 'boas-vindas') check(scenario, `${label}: só o seletor de Tema é interativo`, state.total === 0, state)
  if (step.id === 'conexoes') check(scenario, `${label}: botão "Entrar" desabilitado`, state.total === 1 && state.enabled.length === 0, state)
  if (step.id === 'projeto') check(scenario, `${label}: cartões, origem e "Começar de" desabilitados`, state.total > 0 && state.enabled.length === 0 && state.groups.length === 2 && state.groups.every((group) => group.ariaDisabled === 'true'), state)
  if (step.id === 'time') check(scenario, `${label}: cartões de modelo desabilitados; só "Começar sem time" habilitado`, state.enabled.length === 1 && state.enabled[0] === 'Começar sem time', state)
  if (step.id === 'pedido') check(scenario, `${label}: campo e sugestões desabilitados`, state.total > 0 && state.enabled.length === 0, state)
}

async function scenarioSteps(viewport) {
  const scenario = `Telas ${viewport.width}x${viewport.height}`
  await openOnboarding()
  const app = await pageRun(pageAppState)
  check(scenario, 'Novo projeto do trilho abre o onboarding (h1 "Boas-vindas à Vokia")', app.h1 === STEPS[0].title, app)
  check(scenario, 'sem trilho: nenhum nav Workspace, nenhum botão do trilho e --rail-w 0px', !app.railPresent && app.railButtons === 0 && app.railWidthVar === '0px', app)

  for (let index = 0; index < STEPS.length; index += 1) {
    const step = STEPS[index]
    const label = `${index + 1}/6 ${step.id}`
    const p = await pageRun(pageOnboarding)
    check(scenario, `${label}: título e subtítulo do protótipo`, p.h1 === step.title && p.subtitle === step.subtitle, { h1: p.h1, subtitle: p.subtitle })
    check(scenario, `${label}: marca "Vokia" no topo`, p.brand === 'Vokia', p.brand)
    check(scenario, `${label}: título 34 px com altura de linha 1.1`, p.fontSize === `${TITLE_FONT_PX}px` && Math.abs(parseFloat(p.lineHeight) - TITLE_LINE_HEIGHT_PX) < 0.2, { fontSize: p.fontSize, lineHeight: p.lineHeight })
    check(scenario, `${label}: coluna de ${COLUMN_WIDTH} px centralizada e casca na largura toda (sem coluna vazia do trilho)`, p.columnWidth === COLUMN_WIDTH && Math.abs(p.columnCenterOffset) <= 1 && p.rootLeft === 0 && p.rootWidth === viewport.width, { columnWidth: p.columnWidth, offset: p.columnCenterOffset, rootLeft: p.rootLeft, rootWidth: p.rootWidth })
    check(scenario, `${label}: marcador "${SAMPLE_LABEL}" ${step.sample ? 'visível acima do título' : 'ausente (sem dado de exemplo)'}`, step.sample ? p.marker && p.markerBeforeTitle : !p.marker, { marker: p.marker, before: p.markerBeforeTitle })
    check(scenario, `${label}: "${SKIP_LABEL}" ${step.skip ? 'presente' : 'ausente'}`, step.skip ? p.skip?.text === SKIP_LABEL : p.skip === null, p.skip)
    const expectedFooter = [...(index > 0 ? [{ text: BACK_LABEL, disabled: false }] : []), { text: step.forward, disabled: step.forwardDisabled }]
    check(scenario, `${label}: rodapé ${index === 0 ? 'sem Voltar' : 'com Voltar'} e "${step.forward}" ${step.forwardDisabled ? 'desabilitado' : 'habilitado'}`, JSON.stringify(p.footer) === JSON.stringify(expectedFooter), p.footer)
    checkDisabledControls(scenario, label, step, await pageRun(pageDisabled))
    await pageRun(pageScrollToEnd)
    const end = await pageRun(pageOnboarding)
    check(scenario, `${label}: rolado ao fim, "${step.forward}" não fica sob a barra de ação`, !end.forwardCovered && !end.forwardOverlapsBar && end.forwardBottom <= end.barTop, { forwardTop: end.forwardTop, forwardBottom: end.forwardBottom, barTop: end.barTop, coveredBy: end.forwardHitTag })
    await checkNoHorizontalScroll(scenario, label)
    await screenshot(`${index + 1}-${step.id}-${viewport.width}`)
    if (index < LAST) await advance(index)
  }
}

async function scenarioNavigation() {
  const scenario = 'Navegação'
  await openOnboarding()
  for (let index = 0; index < PEDIDO; index += 1) {
    await clickButton(STEPS[index].forward)
    const now = await pageRun(pageAppState)
    check(scenario, `Continuar em ${STEPS[index].id} leva a ${STEPS[index + 1].id}`, now.h1 === STEPS[index + 1].title, now.h1)
  }
  const before = await pageRun(pageStepContent)
  await clickButton(SEND_LABEL)
  const after = await pageRun(pageStepContent)
  const pedido = await pageRun(pageAppState)
  check(scenario, 'em Pedido, "Enviar para a Iara" desabilitado não faz nada (não envia, não avança)', pedido.h1 === STEPS[PEDIDO].title && after === before, { h1: pedido.h1 })
  for (let index = PEDIDO; index > 0; index -= 1) {
    await clickButton(BACK_LABEL)
    const now = await pageRun(pageAppState)
    check(scenario, `Voltar em ${STEPS[index].id} leva a ${STEPS[index - 1].id}`, now.h1 === STEPS[index - 1].title, now.h1)
  }
  const first = await pageRun(pageOnboarding)
  check(scenario, 'Voltar não existe na primeira tela', !first.footer.some((item) => item.text === BACK_LABEL), first.footer)

  for (let from = 0; from < LAST; from += 1) {
    await openOnboarding()
    await goToStep(from)
    await clickBy('button', SKIP_LABEL)
    const now = await pageRun(pageAppState)
    check(scenario, `"Pular e ir para o fim" em ${STEPS[from].id} vai à tela Pronto`, now.h1 === STEPS[LAST].title, now.h1)
  }

  await openOnboarding()
  await goToStep(3)
  await clickButton('Começar sem time')
  const afterTeamSkip = await pageRun(pageAppState)
  check(scenario, '"Começar sem time" em Time leva a Pedido', afterTeamSkip.h1 === STEPS[PEDIDO].title, afterTeamSkip.h1)

  await openOnboarding()
  await clickBy('button', SKIP_LABEL)
  await clickButton(BACK_LABEL)
  const backFromReady = await pageRun(pageAppState)
  check(scenario, 'Voltar em Pronto (depois de Pular) leva a Pedido, a tela anterior na ordem', backFromReady.h1 === STEPS[PEDIDO].title, backFromReady.h1)

  await openOnboarding()
  await clickBy('button', SKIP_LABEL)
  const tips = await pageRun(pageStepContent)
  check(scenario, 'Pronto lista as três dicas e a nota sobre Configurações', ['A barra de baixo mostra o progresso e as decisões que esperam você', 'Fale direto com um agente no campo da barra', 'Procure páginas, cards e memória', 'O que você pulou fica em Configurações, e dá para rever o onboarding de lá.'].every((text) => tips.includes(text)), tips)
}

async function scenarioRail() {
  const scenario = 'Trilho'
  await openOnboarding()
  await clickBy('button', SKIP_LABEL)
  await clickButton('Abrir o mapa do time')
  const map = await pageRun(pageAppState)
  check(scenario, '"Abrir o mapa do time" leva ao Mapa do time com o trilho de volta (88 px)', map.railPresent && map.active === MAP_LABEL && map.railButtons > 0 && map.railWidthVar === '88px', map)
  check(scenario, 'o Mapa do time não mostra mais o onboarding', map.h1 !== STEPS[LAST].title, map.h1)
  await screenshot('mapa-apos-onboarding')

  await clickBy('button[aria-label]', NEW_PROJECT)
  const reopened = await pageRun(pageAppState)
  const reopenedStep = await pageRun(pageOnboarding)
  check(scenario, 'voltar por "Novo projeto" reinicia na primeira tela, sem trilho', reopened.h1 === STEPS[0].title && !reopened.railPresent && reopenedStep.footer.length === 1, reopened)
  await clickButton('Começar')
  const moving = await pageRun(pageAppState)
  check(scenario, 'depois de reabrir, "Começar" ainda avança (passo não travado)', moving.h1 === STEPS[1].title, moving.h1)

  await clickButton(STEPS[1].forward)
  await clickButton('Continuar')
  await clickButton('Continuar')
  const reachedPedido = await pageRun(pageAppState)
  check(scenario, 'segundo ciclo percorre Conexões → Projeto → Time → Pedido sem travar', reachedPedido.h1 === STEPS[PEDIDO].title && !reachedPedido.railPresent, reachedPedido)
  await clickBy('button', SKIP_LABEL)
  await clickButton('Abrir o mapa do time')
  const second = await pageRun(pageAppState)
  check(scenario, 'segundo ciclo (Pular → Abrir o mapa) também devolve o trilho', second.railPresent && second.active === MAP_LABEL, second)
}

async function scenarioTheme() {
  const scenario = 'Tema'
  await openOnboarding()
  const initial = await pageRun(pageTheme)
  check(scenario, 'estado inicial: Sistema marcado e sem data-theme', initial.attr === null && initial.pressed.join() === THEME_LABELS.system, initial)
  const backgrounds = {}
  for (const choice of ['dark', 'light', 'system']) {
    await clickTheme(choice)
    const now = await pageRun(pageTheme)
    backgrounds[choice] = now.bg
    check(scenario, `escolher ${THEME_LABELS[choice]} aplica o tema e marca só essa opção`, (choice === 'system' ? now.attr === null : now.attr === choice) && now.pressed.join() === THEME_LABELS[choice], now)
  }
  check(scenario, 'o fundo do onboarding muda de fato entre Escuro e Claro', backgrounds.dark !== backgrounds.light, backgrounds)

  for (const choice of ['dark', 'light']) {
    await clickTheme(choice)
    for (let index = 0; index < STEPS.length; index += 1) {
      const step = STEPS[index]
      const contrast = await pageRun(pageContrast)
      const readable = CONTRAST_KEYS.every((key) => contrast[key] === undefined || contrast[key] >= MIN_CONTRAST)
      check(scenario, `${THEME_LABELS[choice]}, ${index + 1}/6 ${step.id}: título, subtítulo, botão final${step.sample ? ' e marcador' : ''} com contraste ≥ ${MIN_CONTRAST}`, readable, contrast)
      if (choice === 'dark') await screenshot(`tema-escuro-${index + 1}-${step.id}`)
      if (index < LAST) await advance(index)
    }
    const kept = await pageRun(pageTheme)
    check(scenario, `${THEME_LABELS[choice]} segue aplicado ao chegar em Pronto`, kept.attr === choice, kept)
    for (let index = LAST; index > 0; index -= 1) await clickButton(BACK_LABEL)
    const first = await pageRun(pageTheme)
    check(scenario, `voltando às Boas-vindas o seletor ainda marca ${THEME_LABELS[choice]}`, first.attr === choice && first.pressed.join() === THEME_LABELS[choice], first)
  }
  await clickTheme('system')
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
      await runScenario(`Telas ${viewport.width}x${viewport.height}`, () => scenarioSteps(viewport))
    }
    await send('Emulation.setDeviceMetricsOverride', VIEWPORTS[0])
    await runScenario('Navegação', scenarioNavigation)
    await runScenario('Trilho', scenarioRail)
    await runScenario('Tema', scenarioTheme)
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
