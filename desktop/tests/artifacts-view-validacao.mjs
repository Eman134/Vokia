// Falhas cobertas: registro corrompido quebrando a tela, valor de tipo errado virando estado inválido,
// armazenamento indisponível derrubando a leitura e a escolha de um projeto vazando para outro.
// Execução: cd desktop && node --test tests/artifacts-view-validacao.mjs
import assert from 'node:assert/strict'
import { beforeEach, test } from 'node:test'
import { createJiti } from 'jiti'

const store = new Map()
const working = {
  getItem: (key) => (store.has(key) ? store.get(key) : null),
  setItem: (key, value) => store.set(key, value),
}
globalThis.localStorage = working

const jiti = createJiti(import.meta.url)
const { readView, writeView, DEFAULT_VIEW } = jiti('../src/features/artifacts/artifactsView.ts')

beforeEach(() => {
  store.clear()
  globalThis.localStorage = working
})

test('sem registro volta ao padrão', () => {
  assert.deepEqual(readView(1), DEFAULT_VIEW)
})

const PRISTINE = { query: '', state: 'todos', layout: 'grade', doneOpen: false }

test('alterar a visão lida não contamina o padrão dos outros projetos', () => {
  const first = readView(1)
  first.query = 'digitado no projeto 1'
  first.layout = 'lista'
  assert.deepEqual(readView(2), PRISTINE)
  globalThis.localStorage = { getItem: () => { throw new Error('bloqueado') }, setItem: () => {} }
  readView(3).query = 'outro'
  assert.deepEqual(readView(4), PRISTINE)
})

test('guarda e devolve a escolha, separada por projeto', () => {
  writeView(1, { query: 'item-49', state: 'concluido', layout: 'lista', doneOpen: true })
  assert.deepEqual(readView(1), { query: 'item-49', state: 'concluido', layout: 'lista', doneOpen: true })
  assert.deepEqual(readView(2), DEFAULT_VIEW)
})

test('registro corrompido ou com tipos errados cai no padrão campo a campo', () => {
  store.set('vokia.artefatos.1', '{não é json')
  assert.deepEqual(readView(1), DEFAULT_VIEW)
  store.set('vokia.artefatos.1', JSON.stringify({ query: 7, state: 'inexistente', layout: 'cartaz', doneOpen: 'sim' }))
  assert.deepEqual(readView(1), DEFAULT_VIEW)
  store.set('vokia.artefatos.1', JSON.stringify({ query: 'abc', layout: 'lista' }))
  assert.deepEqual(readView(1), { ...DEFAULT_VIEW, query: 'abc', layout: 'lista' })
})

test('armazenamento indisponível não derruba a leitura nem a escrita', () => {
  globalThis.localStorage = { getItem: () => { throw new Error('bloqueado') }, setItem: () => { throw new Error('bloqueado') } }
  assert.deepEqual(readView(1), DEFAULT_VIEW)
  assert.doesNotThrow(() => writeView(1, DEFAULT_VIEW))
})
