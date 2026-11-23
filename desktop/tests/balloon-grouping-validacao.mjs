// Validação do card item-54: balões empilhados com timer independente por id e dismissAll
// Execução: cd desktop && node tests/balloon-grouping-validacao.mjs
import assert from 'node:assert/strict'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { createPinia, setActivePinia } = jiti('pinia')
const { nextTick } = jiti('vue')
const { useBalloons } = jiti('../src/features/balloon/useBalloons.ts')
const { ALERT_TIMEOUT_MS, MOMENT_TIMEOUT_MS, REQUEST_TIMEOUT_MS } = jiti('../src/features/balloon/balloonTypes.ts')

const realSetTimeout = globalThis.setTimeout
const realClearTimeout = globalThis.clearTimeout
let now = 0
let timers = []
globalThis.setTimeout = (fn, ms) => {
  const timer = { fn, at: now + ms }
  timers.push(timer)
  return timer
}
globalThis.clearTimeout = (timer) => {
  timers = timers.filter((candidate) => candidate !== timer)
}
async function advance(ms) {
  now += ms
  while (true) {
    const due = timers.filter((candidate) => candidate.at <= now).sort((a, b) => a.at - b.at)[0]
    if (!due) break
    timers = timers.filter((candidate) => candidate !== due)
    due.fn()
  }
  await nextTick()
}

const request = (id) => ({ id: `request:sent:${id}`, kind: 'request', source: 'conversa', title: `Mandado ${id}`, actions: [] })
const results = []
async function test(name, fn) {
  setActivePinia(createPinia())
  timers = []
  now = 0
  try {
    await fn(useBalloons())
    results.push(true)
    console.log(`[PASSOU] ${name}`)
  } catch (error) {
    results.push(false)
    console.error(`[FALHOU] ${name}: ${error.message}`)
  }
}

await test('balões em sequência empilham na lista ativa', async (store) => {
  store.enqueue(request(1))
  store.alert('Task delegada', 'a')
  store.enqueue(request(2))
  await nextTick()
  assert.equal(store.items.length, 3)
  assert.deepEqual(store.items.map((item) => item.id), ['request:sent:1', 'alert:1', 'request:sent:2'])
})

await test('cada item tem timer independente conforme seu kind', async (store) => {
  store.enqueue(request(1))
  store.enqueue({ id: 'm1', kind: 'moment', source: 'conversa', title: 'momento', actions: [] })
  store.enqueue({ id: 'e1', kind: 'error', source: 'conversa', title: 'erro', actions: [] })
  store.enqueue({ id: 'l1', kind: 'limit', source: 'uso do plano', title: 'limite', actions: [] })
  await nextTick()

  await advance(REQUEST_TIMEOUT_MS)
  assert.deepEqual(store.items.map((item) => item.id), ['m1', 'e1', 'l1'])

  await advance(MOMENT_TIMEOUT_MS - REQUEST_TIMEOUT_MS)
  assert.deepEqual(store.items.map((item) => item.id), ['e1', 'l1'])
  assert.equal(store.suggestions.length, 1)
  assert.equal(store.suggestions[0].id, 'm1')
})

await test('alerta fecha sozinho em ALERT_TIMEOUT_MS', async (store) => {
  store.alert('x')
  await nextTick()
  await advance(ALERT_TIMEOUT_MS - 1)
  assert.equal(store.items.length, 1)
  await advance(1)
  assert.equal(store.items.length, 0)
})

await test('hover pausa apenas o balão sob o mouse e reinicia ao sair', async (store) => {
  store.enqueue(request(1))
  store.enqueue(request(2))
  await nextTick()
  store.setHovered('request:sent:1', true)
  await advance(REQUEST_TIMEOUT_MS)
  assert.deepEqual(store.items.map((item) => item.id), ['request:sent:1'])

  store.setHovered('request:sent:1', false)
  await advance(REQUEST_TIMEOUT_MS - 1)
  assert.equal(store.items.length, 1)
  await advance(1)
  assert.equal(store.items.length, 0)
})

await test('dismissAll limpa todos os balões e timers', async (store) => {
  store.enqueue(request(1))
  store.alert('a')
  store.dismissAll()
  assert.equal(store.items.length, 0)
  assert.equal(timers.length, 0)
})

await test('setPanelOpen limpa timers ao abrir e reinicia ao fechar', async (store) => {
  store.enqueue(request(1))
  await nextTick()
  store.setPanelOpen(true)
  assert.equal(timers.length, 0)
  await advance(REQUEST_TIMEOUT_MS * 2)
  assert.equal(store.items.length, 1)

  store.setPanelOpen(false)
  await advance(REQUEST_TIMEOUT_MS)
  assert.equal(store.items.length, 0)
})

await test('replace atualiza item na pilha mantendo posição', async (store) => {
  store.enqueue(request(1))
  store.enqueue(request(2))
  assert.equal(store.replace('request:sent:1', { ...request(1), title: 'novo' }), true)
  assert.equal(store.items[0].title, 'novo')
  assert.equal(store.items.length, 2)
})

globalThis.setTimeout = realSetTimeout
globalThis.clearTimeout = realClearTimeout
if (results.includes(false)) process.exit(1)
console.log('\nTodos passaram')
