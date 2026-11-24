// Validação do card item-376: planos já carregados aparecem ao abrir a tela de Tasks
// Execução: cd desktop && node tests/item-plan-load-on-open-validacao.mjs
import assert from 'node:assert/strict'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { createPinia, setActivePinia } = jiti('pinia')
const { useBacklog } = jiti('../src/features/backlog/useBacklog.ts')
const { useItemPlan } = jiti('../src/features/backlog/useItemPlan.ts')

const ITEM = { id: 'item-1', title: 'Card com plano', type: 'task', state: 'vazio', tags: [] }
const LINKED = { id: 'plano-item-1', name: 'Plano do card', meta: '3 lotes' }

setActivePinia(createPinia())
const backlog = useBacklog()
backlog.items = [ITEM]
backlog.planEntries = [{ itemId: ITEM.id, state: 'vinculado', plan: LINKED, linkedAt: 1 }]

// A store só nasce ao abrir a tela de Tasks, depois que o backlog já carregou os planos.
const itemPlan = useItemPlan()

try {
  assert.equal(itemPlan.stateOf(ITEM.id), 'vinculado')
  assert.equal(itemPlan.lineFor(ITEM), 'Plano do card · 3 lotes')
  assert.deepEqual(itemPlan.plans, [LINKED])
  console.log('[PASSOU] plano salvo aparece ao abrir a tela, sem mover card.')
} catch (error) {
  console.error(`[FALHOU] ${error.message}`)
  process.exit(1)
}
