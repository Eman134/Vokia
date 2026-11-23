import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { test } from 'node:test'

const require = createRequire(import.meta.url)
const jiti = require('jiti')(import.meta.url)
const { isRunnableColumn, showsStartButton } = jiti('../src/features/backlog/runnableColumn.ts')

test('Done esconde o botão Iniciar e bloqueia a execução', () => {
  assert.equal(showsStartButton('Done'), false)
  assert.equal(isRunnableColumn('Done'), false)
})

test('demais colunas mantêm o botão visível', () => {
  for (const column of ['Backlog', 'Ready', 'In progress', 'In review', undefined]) {
    assert.equal(showsStartButton(column), true)
  }
})

test('só Backlog e Ready (e sem coluna) podem rodar', () => {
  assert.equal(isRunnableColumn('Backlog'), true)
  assert.equal(isRunnableColumn('Ready'), true)
  assert.equal(isRunnableColumn(undefined), true)
  assert.equal(isRunnableColumn('In progress'), false)
  assert.equal(isRunnableColumn('In review'), false)
})
