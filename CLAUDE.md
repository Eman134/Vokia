# Vokia — diretrizes de desenvolvimento

Caminhos abaixo partem da raiz do repositório (`desktop/...`).

## Regra de ouro: toda funcionalidade nova avalia MCP e CLI

Ao criar uma funcionalidade, comando Tauri ou entidade, decida se ela pode ser acionada por agentes e por automação. Na dúvida, exponha. A exceção é o que depende de janela nativa bloqueante (`UiOnly`).

Ao criar um comando em `desktop/src-tauri/src/commands/*.rs`:

1. Escreva a lógica em `<nome>_exec(app: &AppHandle, args: <Nome>Args) -> Result<_, String>`, sem depender do runtime Tauri além do `AppHandle`. O `#[tauri::command]` só delega ao `_exec`.
2. Declare os argumentos com `#[derive(Deserialize)]` e `#[serde(rename_all = "camelCase")]`.
3. Registre o comando em `commands/dispatcher.rs`, nos dois lugares:
   - `COMMANDS`: `exposed("nome", "descrição")`, ou `ui_only(...)` quando envolver diálogo nativo bloqueante;
   - `run()`: o braço `"nome" => json_result(modulo::nome_exec(app, parse_args(arguments)?))`.
4. Escreva a descrição em `COMMANDS` com os parâmetros esperados e o que o comando devolve. O `inputSchema` publicado hoje é genérico (`{"type":"object"}`), então a descrição é o que o agente lê para saber chamar.
5. Depois do registro o comando fica disponível sem código extra:
   - MCP: `commands/list` e `commands/call`, com `tools/call` como fallback pelo prefixo `app:`;
   - CLI: `vokia-cli cmd <nome>` e `vokia-cli commands` para listar.

Comandos `UiOnly` respondem com erro explícito via MCP/CLI. Não os registre como `exposed` só para "completar" a lista.

## Arquitetura e responsabilidades

- `src-tauri/src/main.rs`: ponto de entrada mínimo (hoje ~27 linhas, só declara módulos e chama `app::run()`). Não ponha comandos nem setup inline.
- `src-tauri/src/app/mod.rs`: builder e handlers do Tauri. `app/bootstrap.rs`: setup e registries.
- `src-tauri/src/commands/*.rs`: lógica nas funções `_exec`; `dispatcher.rs` é o catálogo único de exposição.
- `src-tauri/src/mcp/`: mantenha `tools/list` enxuto. Os agentes da orquestração pagam esses tokens a cada sessão. Funcionalidade que não precise estar sempre à vista entra por `commands/list` e `commands/call`, não como tool nova.
- `src-tauri/src/bin/vokia_cli.rs` (`vokia-cli`): CLI oficial para automação, testes e agentes locais.

## Frontend (Vue 3 + Vite + Tailwind v4)

- Use os tokens de `src/styles/tokens.css` (`text-ink`, `text-muted`, `bg-surface`, `border-line`, `accent`, `ok`...). Eles usam `light-dark()` e já servem os temas claro e escuro. Não escreva cores fixas.
- Botão habilitado: `cursor-pointer`. Desabilitado: `disabled:cursor-not-allowed`.
- Modal e popover: `Escape` fecha, clique fora fecha e o foco é tratado (`focus-within`, devolver o foco a quem abriu). Referências: o popup de limpar conversa em `features/team/Conversation.vue` e o diálogo de canal em `AgentList.vue`.
- Constantes nomeadas para classes repetidas e textos, como nos componentes existentes.
- Sem teste de componente para detalhes visuais; valide no navegador.

## Verificação

- Rust: `cd desktop/src-tauri && cargo check --bins` e `cargo test --quiet -- --skip live_session`.
- Frontend: `cd desktop && npm run typecheck`.

## Git

- Conventional commits objetivos: `<tipo>(<escopo>): <assunto>` (até 50 caracteres, imperativo).
- Sem trailers como `Co-Authored-By`.
- Commits atômicos; não misture mudanças não relacionadas.
