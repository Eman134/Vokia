// Validação da Etapa ART-5 (exportar e importar planos: Markdown, bloco vokia-plano e protocolo)
// Execução: cd desktop && node tests/plan-markdown-validacao.mjs
import assert from 'node:assert/strict'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)

const { planToMarkdown, extractPlanBlock, titleFromText, planFileName } = jiti(
  '../src/features/artifacts/planMarkdown.ts',
)
const { parseProposal, serializeProposal, buildImportPrompt, PLAN_READY_MARKER } = jiti(
  '../src/features/backlog/planProposal.ts',
)

console.log('=== Iniciando Validação da Etapa ART-5 (Markdown de planos) ===\n')

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

const PROPOSAL = [
  'Entrega o login completo.',
  'Com OAuth e sessão persistente.',
  '#plano-pronto',
  '#estimativa ~3 dias',
  '#lote 1|Base|Base pronta|Criar tabela; Criar API',
  '#roteiro 1|quem:hefesto~modelo:claude-sonnet~ctx:Usa o banco atual~arq:src/db.rs, src/api.rs~passo:Criar migração~passo:Expor rota~pronto:Testes passam~nao:Não mexer na UI',
  '#lote 2|Tela|Tela funcionando',
  '#risco Prazo curto',
  '#etiquetas auth, ui',
].join('\n')

function card(overrides = {}) {
  return {
    id: 'item-7',
    title: 'Login com OAuth',
    who: '',
    column: 'Backlog',
    estimate: '',
    ...overrides,
  }
}

function exported(rawText, extra = {}) {
  return planToMarkdown({
    item: card(),
    proposal: parseProposal(rawText),
    rawText,
    stages: [card({ id: 'plano-item-7.1', title: 'Base', who: 'hefesto', column: 'In progress', parent: 'item-7' })],
    criteria: [
      { id: 'c1', text: 'Base pronta', done: true },
      { id: 'c2', text: 'Tela funcionando', done: false },
    ],
    state: 'vinculado',
    ...extra,
  })
}

// -------------------------------------------------------------
// Ida e volta
// -------------------------------------------------------------
test('planToMarkdown — documento legível traz título, etapas com quem/modelo/status, roteiro, critérios e riscos', () => {
  const lines = exported(PROPOSAL).split('\n')
  assert.equal(lines[0], '# Login com OAuth')
  assert.equal(lines[2], 'Card: item-7 · Estado: pronto · Estimativa: ~3 dias')
  for (const expected of [
    '### 1. Base',
    '- Quem: hefesto',
    '- Modelo: Sonnet',
    '- Status: em andamento',
    '- Meta: Base pronta',
    '  - Criar API',
    '#### Roteiro',
    '- Contexto: Usa o banco atual',
    '- Arquivos: src/db.rs, src/api.rs',
    '  2. Expor rota',
    '  - Testes passam',
    '  - Não mexer na UI',
    '### 2. Tela',
    '- Status: proposta',
    '- [x] Base pronta',
    '- [ ] Tela funcionando',
    '- Prazo curto',
    'Etiquetas: auth, ui',
  ]) {
    assert.ok(lines.includes(expected), `linha ausente: "${expected}"`)
  }
})

test('extractPlanBlock — devolve exatamente o protocolo exportado e parseProposal reconstrói o mesmo plano', () => {
  const block = extractPlanBlock(exported(PROPOSAL))
  assert.equal(block, PROPOSAL)
  assert.deepEqual(parseProposal(block), parseProposal(PROPOSAL))
})

test('extractPlanBlock — arquivo salvo com CRLF (Windows) extrai o mesmo protocolo', () => {
  const block = extractPlanBlock(exported(PROPOSAL).replace(/\n/g, '\r\n'))
  assert.equal(block, PROPOSAL)
})

test('serializeProposal — é o inverso de parseProposal, com #roteiro e sem perda', () => {
  const parsed = parseProposal(PROPOSAL)
  const serialized = serializeProposal(parsed)
  assert.deepEqual(parseProposal(serialized), parsed)
  assert.equal(serializeProposal(parseProposal(serialized)), serialized, 'serializar de novo não muda o texto')
  assert.equal(serialized.split('\n')[0], PLAN_READY_MARKER)
  assert.deepEqual(parsed.lots[0].script, {
    who: 'hefesto',
    model: 'claude-sonnet',
    block: {
      context: 'Usa o banco atual',
      files: ['src/db.rs', 'src/api.rs'],
      steps: ['Criar migração', 'Expor rota'],
      done: ['Testes passam'],
      limits: ['Não mexer na UI'],
    },
  })
})

test('serializeProposal — separadores do protocolo dentro de campos não criam campos nem entradas extras', () => {
  const parsed = parseProposal(PROPOSAL)
  parsed.lots[0].title = 'Base | parte 1'
  parsed.lots[0].meta = 'Base\npronta'
  parsed.lots[0].script.block.context = 'ctx com ~ til'
  parsed.lots[0].script.block.files = ['a,b.rs']
  parsed.tags = ['auth,web']

  const reparsed = parseProposal(serializeProposal(parsed))

  assert.equal(reparsed.lots.length, 2)
  assert.equal(reparsed.lots[0].title, 'Base   parte 1')
  assert.equal(reparsed.lots[0].meta, 'Base pronta')
  assert.equal(reparsed.lots[0].tasks, 'Criar tabela; Criar API')
  assert.equal(reparsed.lots[0].script.block.context, 'ctx com   til')
  assert.deepEqual(reparsed.lots[0].script.block.files, ['a b.rs'])
  assert.deepEqual(reparsed.lots[0].script.block.steps, ['Criar migração', 'Expor rota'])
  assert.deepEqual(reparsed.tags, ['auth web'])
})

test('serializeProposal — renumera lotes pela ordem e o #roteiro acompanha o lote certo', () => {
  const parsed = parseProposal(PROPOSAL)
  parsed.lots.reverse()
  const reparsed = parseProposal(serializeProposal(parsed))
  assert.deepEqual(reparsed.lots.map((lot) => [lot.n, lot.title, lot.script?.who]), [
    [1, 'Tela', undefined],
    [2, 'Base', 'hefesto'],
  ])
})

// -------------------------------------------------------------
// Cerca adaptativa
// -------------------------------------------------------------
test('cerca do bloco — sem crases no plano usa 4 crases', () => {
  const lines = exported(PROPOSAL).split('\n')
  assert.ok(lines.includes('````vokia-plano'), 'abertura com 4 crases')
  assert.equal(lines.filter((line) => line === '````').length, 1, 'fechamento com 4 crases')
})

test('cerca do bloco — ``` e ```` no resumo e no roteiro fazem a cerca crescer e o bloco sai inteiro', () => {
  const tricky = [
    'Rode isto antes:',
    '```bash',
    'npm run build',
    '```',
    'E este exemplo de Markdown:',
    '````',
    '```js',
    'x()',
    '```',
    '````',
    '#plano-pronto',
    '#lote 1|Build|Build verde',
    '#roteiro 1|ctx:rodar ```npm test``` antes~passo:conferir',
  ].join('\n')
  const markdown = exported(tricky)

  assert.ok(markdown.split('\n').includes('`````vokia-plano'), 'abertura com 5 crases')
  const block = extractPlanBlock(markdown)
  assert.equal(block, tricky)
  assert.deepEqual(parseProposal(block), parseProposal(tricky))
})

test('extractPlanBlock — sem bloco vokia-plano, bloco vazio ou sem fechamento devolve undefined', () => {
  assert.equal(extractPlanBlock('# Plano\n\n```md\n#lote 1|A|B\n```'), undefined)
  assert.equal(extractPlanBlock('````vokia-plano\n\n````'), undefined)
  assert.equal(extractPlanBlock('````vokia-plano\n#lote 1|A|B\n```'), undefined)
})

// -------------------------------------------------------------
// titleFromText e planFileName
// -------------------------------------------------------------
test('titleFromText — usa o primeiro "# Título" e ignora "##" e marcadores do protocolo', () => {
  assert.equal(titleFromText('#plano-pronto\n## Fase 1\n#  Plano de Login  #\n# Outro', 'x.md'), 'Plano de Login')
  assert.equal(titleFromText(exported(PROPOSAL), 'ignorado.md'), 'Login com OAuth')
})

test('titleFromText — sem título cai no nome do arquivo sem extensão', () => {
  assert.equal(titleFromText('#plano-pronto\n#lote 1|A|B', 'plano-item-7-login.md'), 'plano-item-7-login')
  assert.equal(titleFromText('', 'notas.da.fase.txt'), 'notas.da.fase')
  assert.equal(titleFromText('texto', 'sem-extensao'), 'sem-extensao')
})

test('planFileName — plano-<card>-<slug>.md sem caracteres que o Windows recusa', () => {
  assert.equal(
    planFileName(card({ id: 'item-7', title: 'Ação: "Login"/SSO? <v2> | *já*' })),
    'plano-item-7-acao-login-sso-v2-ja.md',
  )
  assert.equal(planFileName(card({ id: 'a:b?c', title: 'Plano' })), 'plano-a-b-c-plano.md')
  assert.equal(planFileName(card({ id: 'item-7', title: '???' })), 'plano-item-7.md')
})

test('planFileName — slug longo é cortado em 60 sem traço sobrando na ponta', () => {
  const name = planFileName(card({ id: 'item-7', title: `${'palavra '.repeat(12)}fim` }))
  const slug = name.slice('plano-item-7-'.length, -'.md'.length)
  assert.ok(slug.length <= 60, slug)
  assert.ok(!slug.endsWith('-') && !slug.startsWith('-'), slug)
  assert.ok(/^[a-z0-9-]+$/.test(slug), slug)
})

// -------------------------------------------------------------
// buildImportPrompt
// -------------------------------------------------------------
test('buildImportPrompt — título, texto livre e termina na instrução do protocolo', () => {
  const free = '## Fase 1\nMontar a API\n## Fase 2\nMontar a tela'
  const prompt = buildImportPrompt('Plano do Notion', free)
  const paragraphs = prompt.split('\n\n')

  assert.equal(paragraphs[0], 'Plano "Plano do Notion".')
  assert.ok(prompt.includes(`Plano original:\n${free}`), prompt)
  const last = paragraphs.at(-1)
  for (const marker of ['#plano-pronto', '#lote', '#roteiro']) {
    assert.ok(last.includes(marker), `a instrução final deveria citar ${marker}`)
  }
  assert.ok(prompt.indexOf(free) < prompt.indexOf(last), 'o texto vem antes da instrução final')
})

// -------------------------------------------------------------
// Verificação final
// -------------------------------------------------------------
console.log(`\n=== Resumo dos Testes ART-5 ===`)
const passedCount = results.filter((r) => r.passed).length
const failedCount = results.filter((r) => !r.passed).length
console.log(`Total: ${results.length} | Passaram: ${passedCount} | Falharam: ${failedCount}`)

if (failedCount > 0) {
  process.exit(1)
} else {
  console.log('Todos os testes de Markdown de planos passaram com sucesso!')
}
