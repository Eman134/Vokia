// Falhas cobertas: resultado com verbo trocado, negado/expirado sem marca de falha, comando fora do chip e pedido sem agente.
// Execução: cd desktop && node --test tests/permission-row-validacao.mjs
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { permissionResultRow, permissionRequestRow } = jiti('../src/features/team/permissionSummary.ts')

const result = (outcome) => ({ type: 'permission-result', agent: 'vexa', detail: 'npm test', outcome, time: 1 })

test('cada resultado vira a linha com o verbo e a marca de falha certos', () => {
  const expected = {
    allowed: ['Aprovado', false, 'check'],
    denied: ['Negado', true, 'warning'],
    expired: ['Expirou', true, 'warning'],
    'project-rule': ['Regra do projeto', false, 'check'],
    auto: ['Modo auto', false, 'check'],
    bypass: ['Modo bypass', false, 'check'],
  }
  for (const [outcome, [verb, failed, icon]] of Object.entries(expected)) {
    const row = permissionResultRow(result(outcome))
    assert.deepEqual([row.verb, row.failed, row.icon], [verb, failed, icon], outcome)
    assert.equal(row.chip.label, 'npm test')
  }
})

test('pedido pendente mostra agente, ferramenta e comando inteiro no tooltip', () => {
  const detail = 'git push --force origin main && rm -rf node_modules'
  const permission = { type: 'permission', id: 'p1', agent: 'vexa', tool: 'Bash', detail, projectId: 1, expiresAt: 0, approvedBefore: 0, suggestAlways: false }
  const row = permissionRequestRow(permission, (id) => (id === 'vexa' ? 'Vexa' : undefined))
  assert.equal(row.verb, 'Vexa quer:')
  assert.equal(row.meta, 'Bash')
  assert.equal(row.chip.title, detail)
  assert.equal(permissionRequestRow(permission, () => undefined).verb, 'vexa quer:')
})
