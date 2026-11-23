// Validação de links file:// no parser de Markdown (item-8)
// Execução: cd desktop && node tests/markdown-file-links-validacao.mjs
import assert from 'node:assert/strict'
import { createJiti } from 'jiti'

const jiti = createJiti(import.meta.url)
const { renderMarkdown } = jiti('../src/features/team/markdown.ts')

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

test('renderiza link file:// Windows como mini card inline com ícone, nome e title completo', () => {
  const html = renderMarkdown('Veja [markdown.ts](file:///C:/Users/Kayke/Vokia/desktop/src/features/team/markdown.ts)')
  assert.ok(html.includes('data-file-link'), html)
  assert.ok(html.includes('<svg '), html)
  assert.ok(html.includes('title="C:/Users/Kayke/Vokia/desktop/src/features/team/markdown.ts"'), html)
  assert.ok(html.includes('>markdown.ts<'), html)
  assert.ok(!html.includes('<a '), 'não deve renderizar como tag <a>')
})

test('preserva âncora #L... no mini card e no title', () => {
  const html = renderMarkdown('Confira [trecho](file:///C:/repo/src/app.ts#L12-L34) aqui.')
  assert.ok(html.includes('title="C:/repo/src/app.ts#L12-L34"'), html)
  assert.ok(html.includes('>app.ts<'), html)
  assert.ok(html.includes('>#L12-L34<'), html)
})

test('suporta caminhos Unix, localhost e espaços (%20)', () => {
  const unix = renderMarkdown('[notas](file:///home/dev/My%20Notes.md#L5)')
  assert.ok(unix.includes('title="/home/dev/My Notes.md#L5"'), unix)
  assert.ok(unix.includes('>My Notes.md<'), unix)
  assert.ok(unix.includes('>#L5<'), unix)

  const localhost = renderMarkdown('[local](file://localhost/C:/repo/my_file_name.ts)')
  assert.ok(localhost.includes('title="C:/repo/my_file_name.ts"'), localhost)
  assert.ok(localhost.includes('>my_file_name.ts<'), localhost)
  assert.ok(!localhost.includes('<em>'), 'underscores no nome do arquivo não viram itálico')
})

test('mantém links http(s) e mailto funcionando normalmente', () => {
  const html = renderMarkdown('[site](https://example.com) e [email](mailto:a@b.com)')
  assert.ok(html.includes('<a href="https://example.com" target="_blank" rel="noopener noreferrer">site</a>'), html)
  assert.ok(html.includes('<a href="mailto:a@b.com" target="_blank" rel="noopener noreferrer">email</a>'), html)
})

test('mantém escapeHtml e segurança contra XSS em links file:// e código inline', () => {
  const xss = renderMarkdown('[x](file:///C:/dir/"onmouseover="alert(1)/<script>.ts)')
  assert.ok(!xss.includes('<script>'), xss)
  assert.ok(!xss.includes('"onmouseover='), xss)
  assert.ok(xss.includes('&quot;onmouseover=&quot;'), xss)
  assert.ok(xss.includes('&lt;script&gt;.ts'), xss)

  const inlineCode = renderMarkdown('`[a](file:///C:/foo.ts)`')
  assert.ok(inlineCode.includes('<code>[a](file:///C:/foo.ts)</code>'), inlineCode)
  assert.ok(!inlineCode.includes('data-file-link'), inlineCode)
})

const failed = results.filter((r) => !r.passed)
console.log(`\nTotal: ${results.length} | Passaram: ${results.length - failed.length} | Falharam: ${failed.length}`)
if (failed.length > 0) process.exit(1)
