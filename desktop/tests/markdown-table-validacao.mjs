import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { test } from 'node:test'

const require = createRequire(import.meta.url)
const jiti = require('jiti')(import.meta.url)
const { renderMarkdown } = jiti('../src/features/team/markdown.ts')

test('renderMarkdown — tabela GFM básica vira <table> com thead, tbody e formatação inline', () => {
  const md = [
    '| Dimensão | Modo `stream-json` | PTY |',
    '| :--- | :--- | :--- |',
    '| **Parsing** | NDJSON (`serde_json`) | Saída bruta |',
    '| **Turnos** | Evento `result` | Sem marcador |',
  ].join('\n')

  const html = renderMarkdown(md)
  assert.ok(html.startsWith('<div class="overflow-x-auto"><table><thead><tr>'), html)
  assert.ok(html.includes('<th style="text-align:left">Dimensão</th>'), html)
  assert.ok(html.includes('<th style="text-align:left">Modo <code>stream-json</code></th>'), html)
  assert.ok(html.includes('<td style="text-align:left"><strong>Parsing</strong></td>'), html)
  assert.ok(html.includes('<td style="text-align:left">NDJSON (<code>serde_json</code>)</td>'), html)
  assert.ok(html.includes('</tbody></table></div>'), html)
})

test('renderMarkdown — tabela respeita alinhamentos left, center, right e padrão', () => {
  const md = [
    '| A | B | C | D |',
    '| --- | :--- | :---: | ---: |',
    '| 1 | 2 | 3 | 4 |',
  ].join('\n')

  const html = renderMarkdown(md)
  assert.ok(html.includes('<th>A</th>'), html)
  assert.ok(html.includes('<th style="text-align:left">B</th>'), html)
  assert.ok(html.includes('<th style="text-align:center">C</th>'), html)
  assert.ok(html.includes('<th style="text-align:right">D</th>'), html)
  assert.ok(html.includes('<td>1</td><td style="text-align:left">2</td><td style="text-align:center">3</td><td style="text-align:right">4</td>'), html)
})

test('renderMarkdown — pipe dentro de código inline e pipe escapado não dividem célula', () => {
  const md = [
    '| Expressão | Descrição |',
    '| --- | --- |',
    '| `a | b` | Pipe \\| escapado |',
  ].join('\n')

  const html = renderMarkdown(md)
  assert.ok(html.includes('<td><code>a | b</code></td><td>Pipe | escapado</td>'), html)
})

test('renderMarkdown — tabela após parágrafo sem linha em branco separa os blocos e escapa XSS', () => {
  const md = [
    'Resumo:',
    '| Coluna |',
    '| --- |',
    '| <script>alert(1)</script> |',
  ].join('\n')

  const html = renderMarkdown(md)
  assert.ok(html.startsWith('<p>Resumo:</p><div class="overflow-x-auto"><table>'), html)
  assert.ok(html.includes('&lt;script&gt;alert(1)&lt;/script&gt;'), html)
  assert.ok(!html.includes('<script>'), html)
})

test('renderMarkdown — linha com pipe sem separador válido continua parágrafo normal', () => {
  const html = renderMarkdown('a | b\nc | d')
  assert.equal(html, '<p>a | b<br>c | d</p>')
})
