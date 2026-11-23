// Validação do card item-36: balões de conversa não empilham e fecham sozinhos
// Execução: cd desktop && node tests/balloon-grouping-validacao.mjs
import assert from 'node:assert/strict'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { createPinia, setActivePinia } = jiti('pinia')
const { nextTick } = jiti('vue')
const { useBalloons } = jiti('../src/features/balloon/useBalloons.ts')
const { ALERT_TIMEOUT_MS, REQUEST_TIMEOUT_MS } = jiti('../src/features/balloon/balloonTypes.ts')

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
  for (const timer of timers.filter((candidate) => candidate.at <= now)) {
    timers = timers.filter((candidate) => candidate !== timer)
    timer.fn()
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

await test('mensagens de conversa em sequência não empilham e contam', async (store) => {
  store.enqueue(request(1))
  store.alert('Task delegada', 'a')
  store.enqueue(request(2))
  await nextTick()
  assert.equal(store.queue.length, 0)
  assert.equal(store.current.id, 'request:sent:2')
  assert.equal(store.current.count, 3)
})

await test('balão de conversa fecha sozinho no tempo curto', async (store) => {
  store.enqueue(request(1))
  await nextTick()
  await advance(REQUEST_TIMEOUT_MS - 1)
  assert.notEqual(store.current, null)
  await advance(1)
  assert.equal(store.current, null)
})

await test('alerta fecha sozinho em ALERT_TIMEOUT_MS', async (store) => {
  store.alert('x')
  await nextTick()
  await advance(ALERT_TIMEOUT_MS)
  assert.equal(store.current, null)
})

await test('mensagem nova reinicia o tempo', async (store) => {
  store.enqueue(request(1))
  await nextTick()
  await advance(REQUEST_TIMEOUT_MS - 1000)
  store.enqueue(request(2))
  await nextTick()
  await advance(REQUEST_TIMEOUT_MS - 1)
  assert.notEqual(store.current, null)
  await advance(1)
  assert.equal(store.current, null)
})

await test('mouse sobre o balão pausa e ao sair reinicia', async (store) => {
  store.enqueue(request(1))
  await nextTick()
  store.setHovered(true)
  await advance(REQUEST_TIMEOUT_MS * 3)
  assert.notEqual(store.current, null)
  store.setHovered(false)
  await advance(REQUEST_TIMEOUT_MS)
  assert.equal(store.current, null)
})

await test('momento e limite não são agrupados', async (store) => {
  store.enqueue({ id: 'm1', kind: 'moment', source: 'conversa', title: 'a', actions: [] })
  store.enqueue({ id: 'm2', kind: 'moment', source: 'conversa', title: 'b', actions: [] })
  assert.equal(store.queue.length, 1)
})

await test('replace preserva a contagem do grupo', async (store) => {
  store.enqueue(request(1))
  store.enqueue(request(2))
  await nextTick()
  assert.equal(store.replace('request:sent:2', { ...request(2), title: 'novo' }), true)
  assert.equal(store.current.count, 2)
  assert.equal(store.current.title, 'novo')
})

globalThis.setTimeout = realSetTimeout
globalThis.clearTimeout = realClearTimeout
if (results.includes(false)) process.exit(1)
console.log('\nTodos passaram')
