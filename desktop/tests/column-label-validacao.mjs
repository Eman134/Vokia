// Falhas cobertas: coluna ainda em inglês na tela, selo de etapa com rótulo diferente do título da coluna
// e selo do card "Pronto para o time" enquanto a coluna diz outra coisa.
// Execução: cd desktop && node --test tests/column-label-validacao.mjs
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { COLUMN_CHIP } = jiti('../src/features/backlog/backlogLabels.ts')
const { COLUMN_LABEL, MODAL_STATE_CHIP } = jiti('../src/features/backlog/backlogTypes.ts')

const INTERNAL = ['Backlog', 'Ready', 'In progress', 'In review', 'Done']

test('toda coluna tem rótulo em português, diferente do valor interno', () => {
  assert.deepEqual(Object.keys(COLUMN_LABEL).sort(), [...INTERNAL].sort())
  for (const column of INTERNAL) assert.notEqual(COLUMN_LABEL[column], column)
  assert.deepEqual(INTERNAL.map((column) => COLUMN_LABEL[column]), ['A fazer', 'Pronto', 'Em andamento', 'Em revisão', 'Concluído'])
})

test('o selo da etapa diz o mesmo que o título da coluna', () => {
  for (const [column, chip] of Object.entries(COLUMN_CHIP)) {
    assert.equal(chip.label, COLUMN_LABEL[column].toLocaleLowerCase('pt-BR'))
  }
})

test('card com plano vinculado usa o rótulo da coluna Pronto', () => {
  assert.equal(MODAL_STATE_CHIP.vinculado, COLUMN_LABEL.Ready)
})

test('o estado da etapa no desdobramento usa o mesmo rótulo do selo', () => {
  const { MICRO_STATUS_BADGE } = jiti('../src/features/backlog/backlogLabels.ts')
  for (const chip of Object.values(COLUMN_CHIP)) assert.ok(Object.hasOwn(MICRO_STATUS_BADGE, chip.label), chip.label)
})
