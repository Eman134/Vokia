import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { test } from 'node:test'

const require = createRequire(import.meta.url)
const jiti = require('jiti')(import.meta.url)
const { showsInlinePermission, hasComposerContent } = jiti('../src/features/team/bar/inlinePermission.ts')

const state = (draft, attachments, confirmed) => ({
  flowActive: false,
  pendingCount: 1,
  hasContent: hasComposerContent(draft, attachments, confirmed),
})

test('barra vazia com permissão pendente mostra a inline', () => {
  assert.equal(showsInlinePermission(state('', 0, 0)), true)
})

test('texto, só anexo ou só menção escondem a inline', () => {
  assert.equal(showsInlinePermission(state('oi', 0, 0)), false)
  assert.equal(showsInlinePermission(state('', 1, 0)), false)
  assert.equal(showsInlinePermission(state('', 0, 1)), false)
})

test('a inline volta ao esvaziar a barra', () => {
  assert.equal(showsInlinePermission(state('oi', 1, 1)), false)
  assert.equal(showsInlinePermission(state('', 0, 0)), true)
})

test('sem permissão pendente ou com fluxo ativo a inline não aparece', () => {
  assert.equal(showsInlinePermission({ ...state('', 0, 0), pendingCount: 0 }), false)
  assert.equal(showsInlinePermission({ ...state('', 0, 0), flowActive: true }), false)
})
