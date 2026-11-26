import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { beforeEach, test } from 'node:test'

const require = createRequire(import.meta.url)
const jiti = require('jiti')(import.meta.url)
const { createPinia, setActivePinia } = jiti('pinia')

const { useTriage } = jiti('../src/features/backlog/useTriage.ts')
const { useItemPlan } = jiti('../src/features/backlog/useItemPlan.ts')
const { useProjects } = jiti('../src/features/projects/useProjects.ts')

function deferred() {
  let resolve
  let reject
  const promise = new Promise((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

function flushMicrotasks() {
  return new Promise((resolve) => setTimeout(resolve, 0))
}

let triage
let itemPlan
let projects

beforeEach(() => {
  setActivePinia(createPinia())
  projects = useProjects()
  itemPlan = useItemPlan()
  triage = useTriage()
})

test('enqueueBatch executa itens sequencialmente 1 a 1', async () => {
  const started = []
  const gates = {
    '001': deferred(),
    '002': deferred(),
  }

  itemPlan.requestAndWait = async (id) => {
    started.push(id)
    await gates[id].promise
  }

  triage.enqueueBatch(['001', '002'])
  await flushMicrotasks()

  assert.deepEqual(started, ['001'])
  assert.equal(triage.activeId, '001')
  assert.equal(triage.isRunning('001'), true)
  assert.equal(triage.isQueued('002'), true)
  assert.equal(triage.isBatchActive, true)

  gates['001'].resolve()
  await flushMicrotasks()

  assert.deepEqual(started, ['001', '002'])
  assert.equal(triage.activeId, '002')
  assert.equal(triage.isRunning('002'), true)
  assert.equal(triage.isQueued('002'), false)

  gates['002'].resolve()
  await flushMicrotasks()

  assert.equal(triage.activeId, null)
  assert.equal(triage.queue.length, 0)
  assert.equal(triage.isBatchActive, false)
})

test('enqueueBatch deduplica itens já na fila ou em execução e ignora lista vazia', async () => {
  const started = []
  const gate1 = deferred()
  const gate2 = deferred()

  itemPlan.requestAndWait = async (id) => {
    started.push(id)
    if (id === '001') await gate1.promise
    if (id === '002') await gate2.promise
  }

  triage.enqueueBatch([])
  assert.equal(triage.isBatchActive, false)

  triage.enqueueBatch(['001', '002'])
  await flushMicrotasks()

  // '001' está ativo, '002' na fila; ambos devem ser ignorados ao reenfileirar
  triage.enqueueBatch(['001', '002', '003'])
  assert.deepEqual(triage.queue, ['002', '003'])

  gate1.resolve()
  await flushMicrotasks()
  gate2.resolve()
  await flushMicrotasks()

  assert.deepEqual(started, ['001', '002', '003'])
  assert.equal(triage.isBatchActive, false)
})

test('clearBatch limpa fila, zera activeId e impede execução dos pendentes', async () => {
  const started = []
  const gate1 = deferred()

  itemPlan.requestAndWait = async (id) => {
    started.push(id)
    if (id === '001') await gate1.promise
  }

  triage.enqueueBatch(['001', '002', '003'])
  await flushMicrotasks()

  assert.equal(triage.activeId, '001')
  assert.deepEqual(triage.queue, ['002', '003'])

  triage.clearBatch()
  assert.equal(triage.activeId, null)
  assert.deepEqual(triage.queue, [])
  assert.equal(triage.isBatchActive, false)

  gate1.resolve()
  await flushMicrotasks()

  assert.deepEqual(started, ['001'])
})

test('recupera de falha em requestAndWait, limpa activeId e permite novo lote', async () => {
  const started = []
  const gate1 = deferred()

  itemPlan.requestAndWait = async (id) => {
    started.push(id)
    if (id === '001') return gate1.promise
  }

  triage.enqueueBatch(['001', '002'])
  await flushMicrotasks()

  gate1.reject(new Error('falha simulada'))
  await flushMicrotasks()

  assert.equal(triage.activeId, null)

  // Garante que draining foi resetado e novos lotes funcionam
  triage.enqueueBatch(['003'])
  await flushMicrotasks()
  assert.deepEqual(started, ['001', '002', '003'])
  assert.equal(triage.activeId, null)
})
