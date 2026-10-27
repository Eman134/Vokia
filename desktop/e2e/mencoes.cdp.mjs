import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const DEBUG_LIST_URL = 'http://127.0.0.1:9222/json/list'
const APP_URL_PREFIX = 'http://localhost:1420'
const OUT_DIR = process.argv[2] ?? 'e2e/evidencias-mencoes'
const POLL_MS = 50
const THINKING_GRACE_MS = 5000
const IDLE_TIMEOUT_MS = 180000
const BAR_TIMEOUT_MS = 3000
const RECOVERY_TIMEOUT_MS = 30000
const IDLE_STATUS = 'Tudo em dia'
const DEFAULT_PLACEHOLDER = 'Peça algo ao time'
const CTRL_MODIFIER = 2
const INPUT_SELECTOR = 'form input[role="combobox"]'
const CHIP_SELECTOR = 'form span.bg-accent-soft'
const INPUT_EL = `document.querySelector('${INPUT_SELECTOR}')`
const CHIP_REMOVE_EL = `document.querySelector('form button[aria-label^="Remover destinatário"]')`
const OPTION_DAVI_EL = `[...document.querySelectorAll('[role="option"]')].find((option) => option.textContent.includes('Davi'))`
const CARD_DAVI_EL = `[...document.querySelectorAll('aside button')].find((button) => button.textContent.includes('Davi'))`
const KEY_MAP = {
  Enter: { code: 'Enter', vk: 13, text: '\r' },
  Escape: { code: 'Escape', vk: 27 },
  ArrowDown: { code: 'ArrowDown', vk: 40 },
  ArrowUp: { code: 'ArrowUp', vk: 38 },
  Backspace: { code: 'Backspace', vk: 8, commands: ['deleteBackward'] },
}

const SNAPSHOT_JS = `(() => {
  const input = document.querySelector('${INPUT_SELECTOR}')
  const chip = document.querySelector('${CHIP_SELECTOR}')
  return {
    online: input !== null,
    value: input?.value ?? null,
    placeholder: input?.placeholder ?? null,
    disabled: input?.disabled ?? null,
    expanded: input?.getAttribute('aria-expanded') ?? null,
    inputFocused: document.activeElement === input,
    options: [...document.querySelectorAll('[role="listbox"] [role="option"]')].map((option) => ({
      name: option.querySelector('.block.font-semibold')?.textContent.trim(),
      selected: option.getAttribute('aria-selected'),
    })),
    emptyMenu: document.querySelector('[role="listbox"]') === null && document.body.innerText.includes('Nenhum agente'),
    chip: chip ? chip.childNodes[0].textContent.trim() : null,
    status: document.querySelector('form span.w-32')?.textContent.trim() ?? null,
    pressedCards: [...document.querySelectorAll('aside button[aria-pressed="true"]')].map((button) => button.textContent),
    userBubbles: document.querySelectorAll('div.justify-end p').length,
    authors: [...document.querySelectorAll('header span.font-semibold')].map((span) => span.textContent.trim()),
    offlineBar: document.body.innerText.includes('Sem conexão'),
  }
})()`

const invokes = []

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
const snapshot = () => evaluate(SNAPSHOT_JS)
const selectedName = (state) => state.options.find((option) => option.selected === 'true')?.name
const optionNames = (state) => state.options.map((option) => option.name).join(',')
const sendRequests = () => invokes.filter((call) => call.cmd === 'send_request')
const sendRequestCount = () => sendRequests().length
const lastSendRequest = () => sendRequests().at(-1)?.args ?? null
const captionOf = (text) =>
  evaluate(`(() => {
    const bubble = [...document.querySelectorAll('div.justify-end p')].filter((p) => p.textContent.trim() === ${JSON.stringify(text)}).at(-1)
    return bubble?.parentElement.querySelector('span')?.textContent.trim() ?? null
  })()`)
const bubbleExpression = (text) =>
  `[...document.querySelectorAll('div.justify-end p')].some((p) => p.textContent.trim() === ${JSON.stringify(text)})`

const results = []
let send

function check(scenario, name, passed, detail = '') {
  results.push({ scenario, name, passed: Boolean(passed), detail })
}

async function scenario(id, title, body) {
  try {
    await body()
  } catch (error) {
    check(id, `${title} (execução)`, false, error.message)
  }
}

async function connect() {
  const targets = await (await fetch(DEBUG_LIST_URL)).json()
  const target = targets.find((item) => item.type === 'page' && item.url.startsWith(APP_URL_PREFIX))
  if (!target) {
    throw new Error('página da Vokia não encontrada no CDP (suba com WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS)')
  }
  const socket = new WebSocket(target.webSocketDebuggerUrl)
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true })
    socket.addEventListener('error', () => reject(new Error('falha ao conectar no CDP')), { once: true })
  })
  return socket
}

function observeIpc(socket, send) {
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data)
    if (message.method !== 'Fetch.requestPaused') {
      return
    }
    const { requestId, request } = message.params
    if (request.postData) {
      invokes.push({ cmd: new URL(request.url).pathname.slice(1), args: JSON.parse(request.postData) })
    }
    void send('Fetch.continueRequest', { requestId })
  })
}

function createClient(socket) {
  let nextId = 1
  const pending = new Map()
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data)
    const waiter = pending.get(message.id)
    if (waiter === undefined) {
      return
    }
    pending.delete(message.id)
    if (message.error) {
      waiter.reject(new Error(message.error.message))
    } else {
      waiter.resolve(message.result)
    }
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
  const { result, exceptionDetails } = await send('Runtime.evaluate', {
    expression,
    returnByValue: true,
    awaitPromise: true,
  })
  if (exceptionDetails) {
    throw new Error(exceptionDetails.exception?.description ?? exceptionDetails.text)
  }
  return result.value
}

async function waitFor(expression, description, timeoutMs = IDLE_TIMEOUT_MS) {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    if (await evaluate(expression)) {
      return
    }
    await sleep(POLL_MS)
  }
  throw new Error(`tempo esgotado: ${description}`)
}

async function clickElement(elementJs) {
  const point = await evaluate(`(() => {
    const element = ${elementJs}
    if (element === undefined || element === null) return null
    element.scrollIntoView({ block: 'center' })
    const box = element.getBoundingClientRect()
    return { x: box.left + box.width / 2, y: box.top + box.height / 2 }
  })()`)
  if (point === null) {
    throw new Error(`elemento não encontrado: ${elementJs}`)
  }
  const mouse = { x: point.x, y: point.y, button: 'left', clickCount: 1 }
  await send('Input.dispatchMouseEvent', { ...mouse, type: 'mouseMoved' })
  await send('Input.dispatchMouseEvent', { ...mouse, type: 'mousePressed' })
  await send('Input.dispatchMouseEvent', { ...mouse, type: 'mouseReleased' })
}

async function sendKey({ key, code, vk, modifiers = 0, text, commands }) {
  const common = { key, code, windowsVirtualKeyCode: vk, modifiers }
  await send('Input.dispatchKeyEvent', { ...common, type: 'keyDown', text, commands })
  await send('Input.dispatchKeyEvent', { ...common, type: 'keyUp' })
}

const press = (key) => sendKey({ key, ...KEY_MAP[key] })

async function type(text) {
  await send('Input.insertText', { text })
}

async function clearInput() {
  const { value } = await snapshot()
  if (value === '') {
    return
  }
  await sendKey({ key: 'a', code: 'KeyA', vk: 65, modifiers: CTRL_MODIFIER, commands: ['selectAll'] })
  await press('Backspace')
}

async function screenshot(name) {
  const { data } = await send('Page.captureScreenshot', { format: 'png' })
  writeFileSync(join(OUT_DIR, `${name}.png`), Buffer.from(data, 'base64'))
}

async function setOffline(offline) {
  await send('Network.emulateNetworkConditions', {
    offline,
    latency: 0,
    downloadThroughput: -1,
    uploadThroughput: -1,
  })
}

async function expectChip(id, expected) {
  const state = await snapshot()
  check(id, `estado inicial: chip ${expected ?? 'nenhum'}`, state.chip === expected, state.chip)
  return state
}

async function waitForThinking(seen) {
  const deadline = Date.now() + THINKING_GRACE_MS
  while (Date.now() < deadline) {
    const { status } = await snapshot()
    if (status !== null) {
      seen.add(status)
    }
    if (status?.endsWith(' pensando')) {
      return
    }
    await sleep(POLL_MS)
  }
}

async function sampleUntilIdle(seen) {
  const deadline = Date.now() + IDLE_TIMEOUT_MS
  while (Date.now() < deadline) {
    const { status, disabled } = await snapshot()
    if (status !== null) {
      seen.add(status)
    }
    if (status === IDLE_STATUS && disabled === false) {
      return
    }
    await sleep(POLL_MS)
  }
  throw new Error(`tempo esgotado aguardando "${IDLE_STATUS}"`)
}

async function sendText(typed, bubbleText = typed) {
  await clickElement(INPUT_EL)
  const sentBefore = await sendRequestCount()
  await type(typed)
  await press('Enter')
  await waitFor(bubbleExpression(bubbleText), `bolha "${bubbleText}"`)
  const seen = new Set()
  await waitForThinking(seen)
  await sampleUntilIdle(seen)
  const request = (await sendRequestCount()) > sentBefore ? await lastSendRequest() : null
  return { seen, request }
}

async function run() {
  const initial = await snapshot()
  check('pré', 'barra online com campo', initial.online, initial)
  if (!initial.online) {
    return
  }
  if (initial.chip !== null) {
    await clickElement(CHIP_REMOVE_EL)
  }

  await scenario('3', '@ no meio do texto', async () => {
    await expectChip('3', null)
    await clickElement(INPUT_EL)
    await type('oi @d')
    const state = await snapshot()
    check('3', 'lista não abre para @ no meio', state.expanded === 'false' && state.options.length === 0, state.expanded)
    check('3', 'texto fica como digitado', state.value === 'oi @d', state.value)
    await clearInput()
  })

  await scenario('1', 'lista de menção: abrir, setas, Esc, filtro e mouse', async () => {
    await expectChip('1', null)
    await clickElement(INPUT_EL)
    await type('@')
    let state = await snapshot()
    check('1', '@ no início abre lista com Iara e Davi', state.expanded === 'true' && optionNames(state) === 'Iara,Davi', optionNames(state))
    check('1', 'primeiro agente vem destacado', selectedName(state) === 'Iara', selectedName(state))
    await press('ArrowDown')
    check('1', 'seta para baixo destaca Davi', selectedName(await snapshot()) === 'Davi')
    await press('ArrowUp')
    check('1', 'seta para cima volta a Iara', selectedName(await snapshot()) === 'Iara')
    await press('Escape')
    state = await snapshot()
    check('1', 'Esc fecha a lista e mantém o @', state.expanded === 'false' && state.options.length === 0 && state.value === '@', state.value)
    await clearInput()

    await type('@d')
    check('1', 'filtro por prefixo: @d mostra só Davi', optionNames(await snapshot()) === 'Davi')
    await clearInput()
    await type('@DA')
    check('1', 'filtro sem diferenciar maiúsculas: @DA mostra Davi', optionNames(await snapshot()) === 'Davi')
    await clearInput()
    await type('@x')
    state = await snapshot()
    check('1', 'sem resultado mostra "Nenhum agente"', state.emptyMenu && state.options.length === 0, state.emptyMenu)
    await clearInput()

    await type('@')
    await clickElement(OPTION_DAVI_EL)
    state = await snapshot()
    check('1', 'clique no agente cria chip e limpa o @', state.chip === 'Davi' && state.value === '' && state.placeholder === 'Mensagem para Davi', state)
    screenshot('01-clique-cria-chip')
    await clickElement(CHIP_REMOVE_EL)
    check('1', '× remove o chip', (await snapshot()).chip === null)
  })

  await scenario('2', 'Enter com lista aberta não envia', async () => {
    await expectChip('2', null)
    const before = await snapshot()
    await clickElement(INPUT_EL)
    await type('@x')
    await press('Enter')
    let state = await snapshot()
    check('2', 'Enter com "Nenhum agente" não envia', state.userBubbles === before.userBubbles && state.value === '@x', state.value)
    await clearInput()
    await type('@d')
    await press('Enter')
    state = await snapshot()
    check('2', 'Enter com Davi destacado escolhe, não envia', state.userBubbles === before.userBubbles && state.chip === 'Davi' && state.value === '', state.chip)
    await clickElement(CHIP_REMOVE_EL)
  })

  await scenario('4', 'escolher Davi cria chip e envia só ao Davi', async () => {
    await expectChip('4', null)
    await clickElement(INPUT_EL)
    await type('@d')
    await press('Enter')
    const chosen = await snapshot()
    check('4', 'chip "Davi" e placeholder "Mensagem para Davi"', chosen.chip === 'Davi' && chosen.placeholder === 'Mensagem para Davi', { chip: chosen.chip, placeholder: chosen.placeholder })
    screenshot('04a-chip-davi')

    const text = 'Teste E2E menções 1 para Davi'
    const before = await snapshot()
    const { seen, request } = await sendText(text)
    const after = await snapshot()
    check('4', 'pedido vai ao Davi com texto sem menção', request?.to === 'davi' && request?.text === text, request)
    check('4', 'mensagem do usuário mostra "Para Davi"', (await captionOf(text)) === 'Para Davi')
    check('4', 'Davi pensa e Iara não', seen.has('Davi pensando') && ![...seen].some((status) => status.startsWith('Iara')), [...seen])
    check('4', 'nenhum autor novo com Iara no feed', after.authors.filter((author) => author.includes('Iara')).length === before.authors.filter((author) => author.includes('Iara')).length)
    check('4', 'resposta do Davi aparece no feed principal (CLI)', after.authors.filter((author) => author === 'Davi').length > before.authors.filter((author) => author === 'Davi').length)
    check('4', 'chip persiste após envio', after.chip === 'Davi', after.chip)
    screenshot('04b-depois-do-envio')
  })

  await scenario('4b', 'chip persiste no segundo envio', async () => {
    await expectChip('4b', 'Davi')
    const text = 'Teste E2E menções 2 para Davi'
    const { seen, request } = await sendText(text)
    check('4b', 'segunda mensagem vai ao Davi', request?.to === 'davi' && request?.text === text && (await captionOf(text)) === 'Para Davi', request)
    check('4b', 'Iara não pensa', !seen.has('Iara pensando'), [...seen])
    check('4b', 'chip continua Davi', (await snapshot()).chip === 'Davi')
  })

  await scenario('7', 'Backspace remove o chip só com campo vazio', async () => {
    await expectChip('7', 'Davi')
    await clickElement(INPUT_EL)
    await type('xy')
    await press('Backspace')
    let state = await snapshot()
    check('7', 'Backspace com texto apaga um caractere', state.value === 'x', state.value)
    check('7', 'Backspace com texto não remove o chip', state.chip === 'Davi', state.chip)
    await press('Backspace')
    state = await snapshot()
    check('7', 'segundo Backspace apaga o último caractere e o chip fica', state.value === '' && state.chip === 'Davi', { value: state.value, chip: state.chip })
    await press('Backspace')
    state = await snapshot()
    check('7', 'Backspace com campo vazio remove o chip', state.chip === null && state.placeholder === DEFAULT_PLACEHOLDER, { chip: state.chip, placeholder: state.placeholder })
  })

  await scenario('1c', '"@Davi texto" + Enter envia ao Davi e cria o chip', async () => {
    await expectChip('1c', null)
    const body = 'Teste E2E menções 3 (mensagem completa)'
    const { seen, request } = await sendText(`@Davi ${body}`, body)
    const after = await snapshot()
    check('1c', 'pedido vai ao Davi com texto sem "@Davi"', request?.to === 'davi' && request?.text === body, request)
    check('1c', 'mensagem mostra "Para Davi"', (await captionOf(body)) === 'Para Davi')
    check('1c', 'Iara não pensa', !seen.has('Iara pensando'), [...seen])
    check('1c', 'chip Davi criado e persiste', after.chip === 'Davi', after.chip)
  })

  await scenario('6', 'clicar no cartão do Davi escolhe o destinatário', async () => {
    const start = await snapshot()
    if (start.chip !== null) {
      await clickElement(CHIP_REMOVE_EL)
    }
    await expectChip('6', null)
    await clickElement(CARD_DAVI_EL)
    await sleep(150)
    const state = await snapshot()
    check('6', 'cartão Davi vira chip e fica destacado', state.chip === 'Davi' && state.pressedCards.some((card) => card.includes('Davi')), { chip: state.chip, pressed: state.pressedCards })
    check('6', 'placeholder vira "Mensagem para Davi"', state.placeholder === 'Mensagem para Davi', state.placeholder)
    check('6', 'campo recebe foco', state.inputFocused === true, state.inputFocused)
    screenshot('06-cartao-davi')
  })

  await scenario('8', 'rede desligada (simulada via CDP)', async () => {
    await expectChip('8', 'Davi')
    await send('Network.enable')
    try {
      await setOffline(true)
      await waitFor(`!document.querySelector('${INPUT_SELECTOR}')`, 'barra sai com offline', BAR_TIMEOUT_MS)
      const offline = await snapshot()
      check('8', 'offline (simulado): campo some e aparece Sem conexão', offline.offlineBar && !offline.online, offline.offlineBar)
      screenshot('08a-offline-simulado')
    } finally {
      await setOffline(false)
    }
    await waitFor(`!!document.querySelector('${INPUT_SELECTOR}')`, 'barra volta online', RECOVERY_TIMEOUT_MS)
    await waitFor(`document.querySelector('form span.w-32')?.textContent.trim() === '${IDLE_STATUS}'`, 'status volta a Tudo em dia', RECOVERY_TIMEOUT_MS)
    const back = await snapshot()
    check('8', 'ao voltar online, chip Davi intacto', back.chip === 'Davi', back.chip)
    screenshot('08b-volta-online')
  })

  await scenario('5', 'sem chip, envio vai à Iara', async () => {
    const start = await snapshot()
    if (start.chip !== null) {
      await clickElement(CHIP_REMOVE_EL)
    }
    await expectChip('5', null)
    const text = 'Teste E2E menções 4 (sem chip, Iara)'
    const { seen, request } = await sendText(text)
    const after = await snapshot()
    check('5', 'pedido vai à Iara (to=iara)', request?.to === 'iara' && request?.text === text, request)
    check('5', 'Iara pensa', seen.has('Iara pensando'), [...seen])
    check('5', 'mensagem sem "Para"', (await captionOf(text)) === null)
    check('5', 'chip continua ausente', after.chip === null, after.chip)
    screenshot('05-sem-chip-iara')
  })
}

async function main() {
  const socket = await connect()
  send = createClient(socket)
  observeIpc(socket, send)
  await send('Fetch.enable', { patterns: [{ urlPattern: '*ipc.localhost*', requestStage: 'Request' }] })
  mkdirSync(OUT_DIR, { recursive: true })
  try {
    await run()
  } finally {
    await setOffline(false).catch(() => {})
    socket.close()
  }
  for (const result of results) {
    const status = result.passed ? 'PASSOU' : 'FALHOU'
    console.log(`${status} | ${result.scenario} | ${result.name} | ${JSON.stringify(result.detail)}`)
  }
  writeFileSync(join(OUT_DIR, 'resultados.json'), JSON.stringify(results, null, 2))
  process.exitCode = results.every((result) => result.passed) ? 0 : 1
}

main().catch((error) => {
  console.error(error)
  process.exitCode = 1
})
