import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { test } from 'node:test'

const require = createRequire(import.meta.url)
const jiti = require('jiti')(import.meta.url)
const { cardActionFor } = jiti('../src/features/backlog/cardAction.ts')

test('triagem sem triagem em andamento oferece Triar (e Vincular)', () => {
  assert.deepEqual(cardActionFor({}, 'Backlog', 'triagem'), { label: 'Triar', id: 'triar' })
})

test('triagem em andamento não oferece ação', () => {
  assert.equal(cardActionFor({ triageStep: 2 }, 'Backlog', 'triagem'), undefined)
})

test('vazio, proposto, erro e outras colunas', () => {
  assert.equal(cardActionFor({}, 'Backlog', 'vazio').id, 'triar')
  assert.equal(cardActionFor({}, 'Backlog', 'proposto').id, 'revisar')
  assert.equal(cardActionFor({ error: { reason: 'x' } }, 'Backlog', 'vazio'), undefined)
  assert.equal(cardActionFor({}, 'Ready', 'vinculado').id, 'iniciar')
  assert.equal(cardActionFor({}, 'In progress', 'vazio'), undefined)
})
