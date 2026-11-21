// Validação das Etapas ART-3/ART-4 (progresso de checklist, task list no Markdown e tempo relativo dos artefatos)
// Execução: cd desktop && node tests/artifacts-ui-validacao.mjs
import assert from 'node:assert/strict'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)

const { checklistProgress } = jiti('../src/features/artifacts/artifactChecks.ts')
const { renderMarkdown } = jiti('../src/features/team/markdown.ts')
const { updatedAgo } = jiti('../src/features/artifacts/artifactsCopy.ts')

console.log('=== Iniciando Validação das Etapas ART-3/ART-4 (UI de artefatos) ===\n')

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

const CHECKBOX = /<input type="checkbox"[^>]*data-check-index="(\d+)"([^>]*)><span>(.*?)<\/span>/g

// O índice enviado a set_artifact_check sai do data-check-index; o rótulo identifica qual linha recebeu cada índice.
function renderedChecks(body) {
  return [...renderMarkdown(body, [], { taskList: true }).matchAll(CHECKBOX)].map((match) => ({
    index: Number(match[1]),
    checked: match[2].includes('checked'),
    label: match[3],
  }))
}

const MINUTE = 60_000
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR
const NOW = 1_700_000_000_000

// -------------------------------------------------------------
// checklistProgress
// -------------------------------------------------------------
test('checklistProgress — conta abertos e marcados ([x] e [X]) e arredonda o percentual', () => {
  assert.deepEqual(checklistProgress('- [x] a\n- [ ] b\n- [X] c'), { done: 2, total: 3, percent: 67 })
  assert.deepEqual(checklistProgress('  - [x] recuado\n- [ ] b'), { done: 1, total: 2, percent: 50 })
})

test('checklistProgress — corpo vazio ou sem marcadores devolve 0/0 e 0% (sem divisão por zero)', () => {
  assert.deepEqual(checklistProgress(''), { done: 0, total: 0, percent: 0 })
  assert.deepEqual(checklistProgress('# Título\n- item comum\n* [ ] asterisco não conta'), {
    done: 0,
    total: 0,
    percent: 0,
  })
})

test('checklistProgress — ignora marcadores dentro de blocos cercados por crases, com e sem linguagem', () => {
  const body = ['- [x] fora 1', '```md', '- [ ] dentro', '- [x] dentro', '```', '- [ ] fora 2'].join('\n')
  assert.deepEqual(checklistProgress(body), { done: 1, total: 2, percent: 50 })
})

test('checklistProgress — bloco de código sem fechamento esconde o resto do corpo', () => {
  assert.deepEqual(checklistProgress('- [x] a\n```\n- [ ] b\n- [ ] c'), { done: 1, total: 1, percent: 100 })
})

test('checklistProgress — CRLF não impede a contagem', () => {
  assert.deepEqual(checklistProgress('- [x] a\r\n```\r\n- [ ] b\r\n```\r\n- [ ] c\r\n'), {
    done: 1,
    total: 2,
    percent: 50,
  })
})

// -------------------------------------------------------------
// renderMarkdown com taskList
// -------------------------------------------------------------
test('renderMarkdown taskList — "- [ ]" e "- [x]" viram checkbox com data-check-index na ordem do texto', () => {
  assert.deepEqual(renderedChecks('# Plano\n- [ ] a\n- [x] b\n\nTexto\n\n- [X] c'), [
    { index: 0, checked: false, label: 'a' },
    { index: 1, checked: true, label: 'b' },
    { index: 2, checked: true, label: 'c' },
  ])
})

test('renderMarkdown taskList — linhas dentro de bloco de código continuam código e não consomem índice', () => {
  const html = renderMarkdown('- [ ] a\n```\n- [ ] dentro\n```\n- [x] b', [], { taskList: true })
  assert.ok(html.includes('<pre><code>- [ ] dentro</code></pre>'), html)
  assert.deepEqual(renderedChecks('- [ ] a\n```\n- [ ] dentro\n```\n- [x] b'), [
    { index: 0, checked: false, label: 'a' },
    { index: 1, checked: true, label: 'b' },
  ])
})

test('renderMarkdown taskList — HTML no rótulo do item é escapado (sem XSS)', () => {
  const html = renderMarkdown('- [ ] <script>alert(1)</script>\n- [x] <img src=x onerror="alert(2)">', [], {
    taskList: true,
  })
  assert.ok(!html.includes('<script>'), html)
  assert.ok(!html.includes('<img'), html)
  assert.ok(html.includes('&lt;script&gt;alert(1)&lt;/script&gt;'), html)
  assert.ok(html.includes('&lt;img src=x onerror=&quot;alert(2)&quot;&gt;'), html)
})

test('renderMarkdown taskList — citações, "* [ ]" e listas numeradas não viram checkbox', () => {
  assert.deepEqual(renderedChecks('> - [ ] citado\n\n* [ ] asterisco\n\n1. [ ] numerado\n\n- [ ] real'), [
    { index: 0, checked: false, label: 'real' },
  ])
})

test('renderMarkdown sem taskList — mensagens do chat continuam lista comum sem checkbox', () => {
  assert.equal(renderMarkdown('- [ ] a\n- [x] b'), '<ul><li>[ ] a</li><li>[x] b</li></ul>')
})

test('renderMarkdown sem taskList — "```cmd```" na mesma linha não abre bloco nem engole o resto da mensagem', () => {
  const message = 'Rode isto:\n```npm run build```\nDepois confira o resultado.'
  for (const html of [renderMarkdown(message), renderMarkdown(message, [], { taskList: false })]) {
    assert.ok(!html.includes('<pre>'), html)
    assert.ok(html.startsWith('<p>Rode isto:<br>'), html)
    assert.ok(html.includes('<code>npm run build</code>'), html)
    assert.ok(html.endsWith('<br>Depois confira o resultado.</p>'), html)
  }
})

test('renderMarkdown taskList — índices batem com checklistProgress (regra de set_artifact_check) com cerca recuada', () => {
  // O backend (toggle_check) e checklistProgress tratam "  ```" como cerca; o índice clicado precisa apontar o mesmo item.
  const body = ['- [ ] a', '  ```', '- [ ] dentro', '  ```', '- [x] b'].join('\n')
  assert.deepEqual(checklistProgress(body), { done: 1, total: 2, percent: 50 })
  assert.deepEqual(renderedChecks(body), [
    { index: 0, checked: false, label: 'a' },
    { index: 1, checked: true, label: 'b' },
  ])
})

test('renderMarkdown taskList — índices batem com checklistProgress quando a cerca de fechamento tem linguagem', () => {
  // toggle_check alterna em qualquer linha que comece com ```, inclusive "```js" dentro de um bloco aberto.
  const body = ['```', '- [ ] dentro', '```js', '- [x] fora', '```'].join('\n')
  assert.deepEqual(checklistProgress(body), { done: 1, total: 1, percent: 100 })
  assert.deepEqual(renderedChecks(body), [{ index: 0, checked: true, label: 'fora' }])
})

// -------------------------------------------------------------
// updatedAgo
// -------------------------------------------------------------
test('updatedAgo — limites de minuto, hora e dia', () => {
  assert.equal(updatedAgo(NOW - (MINUTE - 1), NOW), 'atualizado agora')
  assert.equal(updatedAgo(NOW - MINUTE, NOW), 'atualizado há 1 min')
  assert.equal(updatedAgo(NOW - (HOUR - 1), NOW), 'atualizado há 59 min')
  assert.equal(updatedAgo(NOW - HOUR, NOW), 'atualizado há 1 h')
  assert.equal(updatedAgo(NOW - (DAY - 1), NOW), 'atualizado há 23 h')
  assert.equal(updatedAgo(NOW - DAY, NOW), 'atualizado há 1 d')
  assert.equal(updatedAgo(NOW - 3 * DAY, NOW), 'atualizado há 3 d')
})

test('updatedAgo — relógio adiantado no autor (updatedAt no futuro) não gera tempo negativo', () => {
  assert.equal(updatedAgo(NOW + 5 * MINUTE, NOW), 'atualizado agora')
})

// -------------------------------------------------------------
// Verificação final
// -------------------------------------------------------------
console.log(`\n=== Resumo dos Testes ART-3/ART-4 ===`)
const passedCount = results.filter((r) => r.passed).length
const failedCount = results.filter((r) => !r.passed).length
console.log(`Total: ${results.length} | Passaram: ${passedCount} | Falharam: ${failedCount}`)

if (failedCount > 0) {
  process.exit(1)
} else {
  console.log('Todos os testes da UI de artefatos passaram com sucesso!')
}
