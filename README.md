# Vokia

Vokia é um app desktop local que orquestra agentes do Claude Code. Você pede algo ao time pela barra de ação: a Iara, tech lead, responde e pode delegar ao Davi, dev frontend, que responde à Iara; a Iara então devolve o resumo.

## Estado atual

MVP. O instalador atual é para Windows x64, no formato NSIS.

## Requisitos

- Node.js e npm
- Rust (toolchain stable)
- Claude Code CLI instalada e no PATH (`npm install -g @anthropic-ai/claude-code`), e logada com `claude auth login`

## Rodar em desenvolvimento

```bash
cd desktop
npm install
npm run tauri dev
```

Em desenvolvimento, a checagem de atualização não roda.

## Build local

```bash
cd desktop
npm run tauri build
```

O instalador NSIS é gerado em `desktop/src-tauri/target/release/bundle/nsis`. O build local não exige chave de assinatura.

## Verificações

```bash
cd desktop
npm run typecheck
npm run build
```

## Publicar uma versão

1. Suba a versão em `desktop/package.json` e mantenha o mesmo número em `desktop/src-tauri/Cargo.toml`.
2. Crie um tag anotado `vX.Y.Z` cuja mensagem contenha as notas de versão: `git tag -a vX.Y.Z -m "<notas>"`.
3. Envie o tag: `git push origin vX.Y.Z`.
4. A Action `release` (`.github/workflows/release.yml`) compila no Windows, assina o instalador e cria um **rascunho** de release com o instalador, o arquivo de assinatura e o `latest.json`. Ela falha se o número do tag não bater com `desktop/package.json`.
5. Confira o rascunho no GitHub e clique em **Publicar**. Só depois disso as instalações existentes enxergam a atualização.

Editar as notas no rascunho não altera o `latest.json`: o texto de notas vem da mensagem do tag.

Secrets necessários, cadastrados em Settings > Secrets and variables > Actions do repositório:

- `TAURI_SIGNING_PRIVATE_KEY`: conteúdo da chave privada de assinatura.
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: senha da chave.

A chave privada nunca vai para o repositório. Detalhes do fluxo de distribuição e atualização estão em [docs/vokia-visao-de-produto.md](docs/vokia-visao-de-produto.md), seção 6.6.

## Estrutura

- `desktop/`: app Tauri 2 com frontend em Vue 3 (`src/`) e núcleo em Rust (`src-tauri/`).
- `docs/`: visão do produto e protótipos (`docs/prototipo/`).
- `.github/workflows/`: workflow de release.

Núcleo Rust (`desktop/src-tauri/src/`):
- `main.rs`: ponto de entrada; declara os módulos e monta o Builder do Tauri.
- `state.rs`: `AppState`, com o caminho da CLI e as sessões dos agentes.
- `commands.rs`: comandos expostos ao frontend (`cli_problem`, `send_request`, `check_connection`); só delegam.
- `window.rs`: janela principal, instância única e relato de erro de janela.
- `tray.rs`: ícone e menu da bandeja.
- `claude.rs`: adaptador da CLI do Claude Code.
- `orchestrator.rs`: fluxo da Iara e do Davi.
- `events.rs`: eventos enviados ao frontend.
- `connectivity.rs`: checagem de alcance da API, sem enviar dados.

Frontend (`desktop/src/`):
- `main.ts` e `App.vue` na raiz; `styles/` com tokens e CSS global; `shell/` com a moldura do app.
- `features/<nome>/`: componentes, store (`useX.ts`), serviço (`xApi.ts`) quando a feature fala com o Tauri, e tipos (ex.: `features/team/events.ts`, que espelha `events.rs`). Hoje: `team`, `updater` e `theme`.
- Camadas em uma direção: componente → store → serviço → Tauri. Só os serviços importam `@tauri-apps`.
- Tokens de design só em `desktop/src/styles/tokens.css`; constantes nomeadas no topo do arquivo que as usa.

## Documentação

- [Visão de produto](docs/vokia-visao-de-produto.md)
