// Falhas cobertas: "ontem" contado por 24h em vez de dia do calendário, data de outro ano sem o ano e texto fora do formato quebrando a linha.
// Execução: cd desktop && node --test tests/activity-time-validacao.mjs
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { activityTime } = jiti('../src/features/backlog/activityTime.ts')

const now = new Date(2026, 10, 24, 0, 5)

test('hoje e ontem seguem o dia do calendário, não as últimas 24 horas', () => {
  assert.equal(activityTime('24/11/2026, 00:01:09', now), 'hoje, 00:01')
  assert.equal(activityTime('23/11/2026, 23:59:00', now), 'ontem, 23:59')
  assert.equal(activityTime('22/11/2026, 23:59:00', now), '22/11, 23:59')
})

test('data de outro ano mostra o ano', () => {
  assert.equal(activityTime('31/12/2025, 15:38:00', now), '31/12/2025, 15:38')
})

test('texto fora do formato volta como veio', () => {
  assert.equal(activityTime('agora há pouco', now), 'agora há pouco')
  assert.equal(activityTime('', now), '')
})
