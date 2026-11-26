import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { test } from 'node:test'

const require = createRequire(import.meta.url)
const jiti = require('jiti')(import.meta.url)
const {
  createEmptySelection,
  toggleItemSelection,
  selectRange,
  applySelectionClick,
  reconcileSelection,
} = jiti('../src/features/backlog/batchSelection.ts')

const ORDERED = ['001', '002', '003', '004', '005']

test('createEmptySelection retorna conjunto vazio e âncora nula', () => {
  const state = createEmptySelection()
  assert.equal(state.selected.size, 0)
  assert.equal(state.anchorId, null)
})

test('toggleItemSelection alterna item sem mutar o Set original', () => {
  const initial = new Set(['001'])
  const added = toggleItemSelection(initial, '003')
  assert.deepEqual([...added], ['001', '003'])
  assert.deepEqual([...initial], ['001'])

  const removed = toggleItemSelection(added, '001')
  assert.deepEqual([...removed], ['003'])
})

test('selectRange seleciona intervalo crescente, decrescente e trata bordas', () => {
  const forward = selectRange(ORDERED, '002', '004', new Set(['001']))
  assert.deepEqual([...forward].sort(), ['001', '002', '003', '004'])

  const backward = selectRange(ORDERED, '004', '002', new Set())
  assert.deepEqual([...backward].sort(), ['002', '003', '004'])

  const missingAnchor = selectRange(ORDERED, '999', '003', new Set())
  assert.deepEqual([...missingAnchor], ['003'])

  const missingTarget = selectRange(ORDERED, '002', '999', new Set(['001']))
  assert.deepEqual([...missingTarget], ['001'])
})

test('applySelectionClick aplica toggle simples, Shift+clique com âncora e Shift+clique sem âncora', () => {
  const s1 = applySelectionClick({
    selected: new Set(),
    anchorId: null,
    orderedIds: ORDERED,
    targetId: '002',
    shiftKey: false,
  })
  assert.deepEqual([...s1.selected], ['002'])
  assert.equal(s1.anchorId, '002')

  const s2 = applySelectionClick({
    selected: s1.selected,
    anchorId: s1.anchorId,
    orderedIds: ORDERED,
    targetId: '004',
    shiftKey: true,
  })
  assert.deepEqual([...s2.selected].sort(), ['002', '003', '004'])
  assert.equal(s2.anchorId, '004')

  const s3 = applySelectionClick({
    selected: s2.selected,
    anchorId: s2.anchorId,
    orderedIds: ORDERED,
    targetId: '004',
    shiftKey: false,
  })
  assert.deepEqual([...s3.selected].sort(), ['002', '003'])
  assert.equal(s3.anchorId, null)

  const s4 = applySelectionClick({
    selected: s3.selected,
    anchorId: null,
    orderedIds: ORDERED,
    targetId: '005',
    shiftKey: true,
  })
  assert.deepEqual([...s4.selected].sort(), ['002', '003', '005'])
  assert.equal(s4.anchorId, '005')
})

test('reconcileSelection remove IDs ausentes, limpa âncora inválida e preserva referência estável', () => {
  const empty = createEmptySelection()
  assert.equal(reconcileSelection(empty, ORDERED), empty)

  const state = { selected: new Set(['001', '003', '005']), anchorId: '003' }
  assert.equal(reconcileSelection(state, ORDERED), state)

  const pruned = reconcileSelection(state, ['001', '005'])
  assert.deepEqual([...pruned.selected].sort(), ['001', '005'])
  assert.equal(pruned.anchorId, null)
})
