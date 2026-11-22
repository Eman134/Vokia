// Validação do Parser TS do Plano e Estrutura de Desdobramento (PLN-MOD-1 / PLN-MOD-4)
// Execução: cd desktop && node tests/modal-plano-parser-validacao.mjs
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createJiti } from 'jiti'

const __dirname = dirname(fileURLToPath(import.meta.url))
const DESKTOP_DIR = join(__dirname, '..')

const jiti = createJiti(import.meta.url)

const {
  parseProposal,
  isFinalReply,
  PLAN_READY_MARKER,
  buildPlanPrompt,
  buildAdjustPrompt,
} = jiti('../src/features/backlog/planProposal.ts')

const {
  breakdownOf,
  modelTierLabel,
  planSummaryLabel,
  linkKind,
} = jiti('../src/features/backlog/planBreakdown.ts')

console.log('=== Iniciando Validação do Parser TS do Plano Compacto (PLN-MOD-1 / PLN-MOD-4) ===\n')

const results = []

function test(name, fn) {
  try {
    fn()
    results.push({ name, passed: true })
    console.log(`[PASSOU] ${name}`)
  } catch (error) {
    results.push({ name, passed: false, error: error.message })
    console.error(`[FALHOU] ${name}: ${error.message}`)
  }
}

// Implementação espelho do validador de contrato de MicroTaskScript.vue
function isScriptValidLocal(script) {
  return (
    Boolean(script) &&
    typeof script.context === 'string' &&
    script.context.trim() !== '' &&
    Array.isArray(script.files) &&
    script.files.length > 0 &&
    Array.isArray(script.steps) &&
    script.steps.length >= 2 &&
    Array.isArray(script.done) &&
    script.done.length > 0 &&
    Array.isArray(script.limits) &&
    script.limits.length > 0
  )
}

// -------------------------------------------------------------
// 1. Parser de proposta com #roteiro completo e sem vazamento para summary
// -------------------------------------------------------------
test('parseProposal extrai lotes e associa #roteiro completo sem vazar linhas para o resumo', () => {
  const proposalText = [
    'Resumo da proposta do plano.',
    'Mais uma linha de explicação.',
    '#plano-pronto',
    '#estimativa ~2 h',
    '#lote 1|Base de Dados|Tabelas criadas|Criar migration; Executar migrate',
    '#roteiro 1|quem:ember~modelo:sonnet~ctx:Configurar PostgreSQL~arq:schema.sql, migrate.ts~passo:Criar tabela~passo:Aplicar índices~pronto:Tabelas acessíveis~nao:Não alterar schema antigo',
    '#lote 2|Endpoints API|Endpoints prontos|Rota GET; Rota POST',
    '#roteiro 2|quem:kairo~modelo:haiku~ctx:Criar rotas Fastify~arq:routes.ts~passo:Definir schema de validação~passo:Implementar handler~pronto:Testes passam~nao:Não expor rota sem auth',
    '#risco Latência de banco',
    '#etiquetas database, api',
  ].join('\n')

  assert.equal(isFinalReply(proposalText), true)

  const parsed = parseProposal(proposalText)

  assert.equal(parsed.summary, 'Resumo da proposta do plano.\nMais uma linha de explicação.')
  assert.equal(parsed.estimate, '~2 h')
  assert.equal(parsed.risk, 'Latência de banco')
  assert.deepEqual(parsed.tags, ['database', 'api'])
  assert.equal(parsed.lots.length, 2)

  // Lote 1
  const lot1 = parsed.lots[0]
  assert.equal(lot1.n, 1)
  assert.equal(lot1.title, 'Base de Dados')
  assert.equal(lot1.meta, 'Tabelas criadas')
  assert.equal(lot1.tasks, 'Criar migration; Executar migrate')
  assert.ok(lot1.script, 'lote 1 deve ter script associado')
  assert.equal(lot1.script.who, 'ember')
  assert.equal(lot1.script.model, 'sonnet')
  assert.equal(lot1.script.block.context, 'Configurar PostgreSQL')
  assert.deepEqual(lot1.script.block.files, ['schema.sql', 'migrate.ts'])
  assert.deepEqual(lot1.script.block.steps, ['Criar tabela', 'Aplicar índices'])
  assert.deepEqual(lot1.script.block.done, ['Tabelas acessíveis'])
  assert.deepEqual(lot1.script.block.limits, ['Não alterar schema antigo'])

  // Lote 2
  const lot2 = parsed.lots[1]
  assert.equal(lot2.n, 2)
  assert.equal(lot2.title, 'Endpoints API')
  assert.ok(lot2.script)
  assert.equal(lot2.script.who, 'kairo')
  assert.equal(lot2.script.model, 'haiku')
  assert.equal(lot2.script.block.steps.length, 2)
})

// -------------------------------------------------------------
// 2. #roteiro posicionado antes do respectivo #lote
// -------------------------------------------------------------
test('parseProposal associa #roteiro mesmo quando declarado antes do #lote correspondente', () => {
  const text = [
    'Resumo limpo.',
    '#plano-pronto',
    '#roteiro 1|quem:axiom~modelo:opus~ctx:Contexto antecipado~arq:config.json~passo:Passo 1~passo:Passo 2~pronto:Pronto~nao:Nenhum',
    '#lote 1|Configuração Inicial|Sistema configurado|Ajustar env',
  ].join('\n')

  const parsed = parseProposal(text)
  assert.equal(parsed.summary, 'Resumo limpo.')
  assert.equal(parsed.lots.length, 1)
  assert.equal(parsed.lots[0].title, 'Configuração Inicial')
  assert.ok(parsed.lots[0].script, 'deve associar script mesmo vindo antes')
  assert.equal(parsed.lots[0].script.who, 'axiom')
  assert.equal(parsed.lots[0].script.model, 'opus')
  assert.equal(parsed.lots[0].script.block.context, 'Contexto antecipado')
})

// -------------------------------------------------------------
// 3. Formas de falha e robustez do parseador de #roteiro
// -------------------------------------------------------------
test('parseProposal ignora graciosamente roteiros malformados sem poluir resumo nem quebrar outros lotes', () => {
  const malformedLines = [
    'Resumo sem poluição.',
    '#plano-pronto',
    '#roteiro', // sem corpo
    '#roteiro   ', // em branco
    '#roteiro sem-pipe', // falta |
    '#roteiro abc|quem:ember', // lote não numérico
    '#roteiro 0|quem:ember', // lote < 1
    '#roteiro 1|campo-sem-dois-pontos', // chave sem valor
    '#roteiro 1|chave_desconhecida:valor', // chave não suportada
    '#roteiro 99|quem:ember~passo:1~passo:2', // lote inexistente
    '#lote 1|Lote Sobrevivente|Meta|Tarefa A',
  ].join('\n')

  const parsed = parseProposal(malformedLines)
  assert.equal(parsed.summary, 'Resumo sem poluição.')
  assert.equal(parsed.lots.length, 1)
  assert.equal(parsed.lots[0].title, 'Lote Sobrevivente')
  assert.equal(parsed.lots[0].script, undefined, 'roteiro inválido não deve ser associado')
})

// -------------------------------------------------------------
// 4. Regressão: proposta antiga sem #roteiro continua idêntica
// -------------------------------------------------------------
test('parseProposal mantém compatibilidade exata com propostas antigas sem #roteiro', () => {
  const legacyProposal = [
    'Proposta legada sem roteiros.',
    'Segunda linha de detalhe.',
    '#plano-pronto',
    '#estimativa 3 dias',
    '#lote 1|Primeiro|Meta 1|T1; T2',
    '#lote 2|Segundo|Meta 2|T3',
    '#risco Prazo apertado',
    '#etiquetas legada, backend',
  ].join('\n')

  const parsed = parseProposal(legacyProposal)
  assert.equal(parsed.summary, 'Proposta legada sem roteiros.\nSegunda linha de detalhe.')
  assert.equal(parsed.estimate, '3 dias')
  assert.equal(parsed.lots.length, 2)
  assert.equal(parsed.lots[0].script, undefined)
  assert.equal(parsed.lots[1].script, undefined)
  assert.equal(parsed.risk, 'Prazo apertado')
  assert.deepEqual(parsed.tags, ['legada', 'backend'])
})

// -------------------------------------------------------------
// 5. Validação das regras de isScriptValid
// -------------------------------------------------------------
test('isScriptValid valida presença de contexto, arquivos, mínimo 2 passos, pronto e não', () => {
  // Contrato do componente Vue verificado estaticamente
  const vueSource = readFileSync(
    join(DESKTOP_DIR, 'src/features/backlog/MicroTaskScript.vue'),
    'utf8',
  )
  assert.ok(vueSource.includes('MIN_SCRIPT_STEPS = 2'))
  assert.ok(vueSource.includes('script.context.trim() !== \'\''))
  assert.ok(vueSource.includes('script.files.length > 0'))
  assert.ok(vueSource.includes('script.steps.length >= MIN_SCRIPT_STEPS'))
  assert.ok(vueSource.includes('script.done.length > 0'))
  assert.ok(vueSource.includes('script.limits.length > 0'))

  const validBlock = {
    context: 'Contexto claro',
    files: ['src/app.ts'],
    steps: ['Passo 1', 'Passo 2'],
    done: ['Feito quando rodar'],
    limits: ['Não quebrar build'],
  }
  assert.equal(isScriptValidLocal(validBlock), true)

  // Falha 1: contexto vazio
  assert.equal(isScriptValidLocal({ ...validBlock, context: '   ' }), false)

  // Falha 2: arquivos vazio
  assert.equal(isScriptValidLocal({ ...validBlock, files: [] }), false)

  // Falha 3: menos de 2 passos
  assert.equal(isScriptValidLocal({ ...validBlock, steps: [] }), false)
  assert.equal(isScriptValidLocal({ ...validBlock, steps: ['Apenas 1 passo'] }), false)

  // Falha 4: done vazio
  assert.equal(isScriptValidLocal({ ...validBlock, done: [] }), false)

  // Falha 5: limits vazio
  assert.equal(isScriptValidLocal({ ...validBlock, limits: [] }), false)
})

// -------------------------------------------------------------
// 6. breakdownOf para plano proposto
// -------------------------------------------------------------
test('breakdownOf gera steps de proposta com IDs <id>.<n>, tarefas divididas e roteiro', () => {
  const item = { id: 'PAY-14', title: 'Pagamento Pix', estimate: '~4 h' }
  const proposal = {
    estimate: '~4 h',
    lots: [
      {
        n: 1,
        title: 'Criar chave Pix',
        meta: 'Chave cadastrada',
        tasks: 'Gerar QR Code; Validar formato',
        script: {
          who: 'ember',
          model: 'sonnet',
          block: {
            context: 'Integração Bacen',
            files: ['pix.ts'],
            steps: ['Passo 1', 'Passo 2'],
            done: ['OK'],
            limits: ['Sem mock'],
          },
        },
      },
      {
        n: 2,
        title: 'Webhook Pix',
        meta: 'Notificação recebida',
        tasks: 'Endpoint POST; Callback',
        // Sem script: fallback
      },
    ],
  }

  const modelOf = (agent) => (agent === 'ember' ? 'claude-sonnet-4-6' : 'haiku')

  const breakdown = breakdownOf(item, proposal, [], modelOf)
  assert.equal(breakdown.estimate, '~4 h')
  assert.equal(breakdown.steps.length, 2)
  assert.equal(breakdown.progress, undefined, 'proposta sem stages não tem progress')

  // Passo 1 com script
  const step1 = breakdown.steps[0]
  assert.equal(step1.id, 'PAY-14.1')
  assert.equal(step1.n, 1)
  assert.equal(step1.title, 'Criar chave Pix')
  assert.equal(step1.status, 'proposta')
  assert.equal(step1.who, 'ember')
  assert.equal(step1.model, 'sonnet')
  assert.equal(step1.goal, 'Chave cadastrada')
  assert.deepEqual(step1.tasks, ['Gerar QR Code', 'Validar formato'])
  assert.ok(step1.script)
  assert.equal(step1.script.context, 'Integração Bacen')

  // Passo 2 sem script
  const step2 = breakdown.steps[1]
  assert.equal(step2.id, 'PAY-14.2')
  assert.equal(step2.n, 2)
  assert.equal(step2.title, 'Webhook Pix')
  assert.equal(step2.status, 'proposta')
  assert.equal(step2.script, undefined)
  assert.deepEqual(step2.tasks, ['Endpoint POST', 'Callback'])
})

// -------------------------------------------------------------
// 7. breakdownOf para plano vinculado com stages
// -------------------------------------------------------------
test('breakdownOf mapeia stages vinculadas plano-<item>.<n> com status da coluna e progresso', () => {
  const item = { id: 'PAY-14', title: 'Pagamento Pix' }
  const proposal = {
    estimate: '~2 h',
    lots: [
      { n: 1, title: 'Etapa 1', meta: 'M1', tasks: 'T1' },
      { n: 2, title: 'Etapa 2', meta: 'M2', tasks: 'T2' },
    ],
  }
  const stages = [
    {
      id: 'plano-PAY-14.1',
      title: 'Etapa 1 Feita',
      column: 'Done',
      who: 'axiom',
    },
    {
      id: 'plano-PAY-14.2',
      title: 'Etapa 2 Andando',
      column: 'In progress',
      who: 'kairo',
    },
  ]

  const modelOf = (agent) => (agent === 'axiom' ? 'opus' : 'haiku')

  const breakdown = breakdownOf(item, proposal, stages, modelOf)
  assert.equal(breakdown.steps.length, 2)

  const s1 = breakdown.steps[0]
  assert.equal(s1.id, 'PAY-14.1')
  assert.equal(s1.title, 'Etapa 1 Feita')
  assert.equal(s1.status, 'feito')
  assert.equal(s1.who, 'axiom')
  assert.equal(s1.model, 'opus')
  assert.equal(s1.stageId, 'plano-PAY-14.1')

  const s2 = breakdown.steps[1]
  assert.equal(s2.id, 'PAY-14.2')
  assert.equal(s2.title, 'Etapa 2 Andando')
  assert.equal(s2.status, 'em andamento')
  assert.equal(s2.who, 'kairo')
  assert.equal(s2.model, 'haiku')

  // Progresso calculado via planProgress (1 de 2 feito = 50%)
  assert.ok(breakdown.progress)
  assert.equal(breakdown.progress.total, 2)
  assert.equal(breakdown.progress.done, 1)
  assert.equal(breakdown.progress.percent, 50)
})

// -------------------------------------------------------------
// 8. Funções utilitárias: modelTierLabel, planSummaryLabel, linkKind
// -------------------------------------------------------------
test('funções utilitárias modelTierLabel, planSummaryLabel e linkKind funcionam conforme o contrato', () => {
  // modelTierLabel
  assert.equal(modelTierLabel('claude-haiku-4'), 'Haiku')
  assert.equal(modelTierLabel('sonnet-3.5'), 'Sonnet')
  assert.equal(modelTierLabel('OPUS-4'), 'Opus')
  assert.equal(modelTierLabel('gemini-pro'), 'gemini-pro')
  assert.equal(modelTierLabel(undefined), undefined)
  assert.equal(modelTierLabel('   '), undefined)

  // planSummaryLabel
  assert.equal(planSummaryLabel(1), '1 micro-tarefa')
  assert.equal(planSummaryLabel(4), '4 micro-tarefas')
  assert.equal(planSummaryLabel(1, '~30 min'), '1 micro-tarefa · ~30 min')
  assert.equal(planSummaryLabel(4, '~2 h'), '4 micro-tarefas · ~2 h')

  // linkKind
  assert.equal(linkKind('plano-PAY-14'), 'plano')
  assert.equal(linkKind('adr-002-arquitetura'), 'ADR')
  assert.equal(linkKind('doc-guia-api'), 'doc')
  assert.equal(linkKind('spec-pix'), 'spec')
  assert.equal(linkKind('issue-42'), 'link')
})

// -------------------------------------------------------------
// Nota do usuário no prompt do plano
// -------------------------------------------------------------
const NOTE_PREFIX = 'Instruções do usuário (têm prioridade sobre o resto):'
const baseItem = { id: 'BL-12', title: 'Checkout com Pix', description: 'Pagar com QR.', tags: [] }
const lastParagraph = (prompt) => prompt.split('\n\n').at(-1)

test('Prompt sem nota (ausente, vazia ou só espaços) fica idêntico', () => {
  const plain = buildPlanPrompt(baseItem, 'completo')
  assert.ok(!plain.includes(NOTE_PREFIX))
  assert.equal(buildPlanPrompt({ ...baseItem, planNote: '' }, 'completo'), plain)
  assert.equal(buildPlanPrompt({ ...baseItem, planNote: ' \n\t ' }, 'completo'), plain)
  const adjust = buildAdjustPrompt(baseItem, '#plano-pronto', 'junte os lotes')
  assert.equal(buildAdjustPrompt({ ...baseItem, planNote: '   ' }, '#plano-pronto', 'junte os lotes'), adjust)
})

test('Nota entra no pedido com quebras de linha e antes da instrução final', () => {
  const note = 'Reaproveite o fluxo do cartão.\nNão mexa no schema de pedidos.'
  const prompt = buildPlanPrompt({ ...baseItem, planNote: `  ${note}\n` }, 'rapido')
  assert.ok(prompt.includes(`${NOTE_PREFIX}\n${note}`))
  assert.ok(lastParagraph(prompt).startsWith('Quando terminar'))
  assert.ok(prompt.indexOf(NOTE_PREFIX) > prompt.indexOf('Seja rápido'))
})

test('Nota entra no ajuste antes da instrução final', () => {
  const prompt = buildAdjustPrompt({ ...baseItem, planNote: 'QR expira em 30 min.' }, '#plano-pronto', 'junte os lotes')
  assert.ok(prompt.includes(`${NOTE_PREFIX}\nQR expira em 30 min.`))
  assert.ok(lastParagraph(prompt).startsWith('Quando terminar'))
})

// -------------------------------------------------------------
// Resumo dos testes
// -------------------------------------------------------------
const passed = results.filter((r) => r.passed).length
const total = results.length
console.log(`\n=== Resumo dos testes do Parser do Plano: ${passed}/${total} passaram ===`)

if (passed !== total) {
  process.exit(1)
}
