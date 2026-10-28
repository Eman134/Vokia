# Roadmap de implementação · Vokia

Ordem dos sistemas e das execuções para implementar o backlog da Vokia no modo AI-made: Maestro, Planejador, Implementador, Tester e Reviewer. Não há datas, prazos nem estimativas de tempo. A ordem vem das dependências declaradas em cada requisito de [backlog/](backlog/), das decisões aceitas em [PENDENCIAS.md](PENDENCIAS.md) e das fases da [visão de produto](vokia-visao-de-produto.md#12-roadmap).

## Status das fases

Situação conferida no código em 2026-10-26, por trecho com `rg`. Estados possíveis: ⏳ Pendente, 📝 Planejando, 🔨 Implementando, 🧪 Testando, 🔍 Em revisão, ⏸️ Aguardando decisão, 🚫 Bloqueada e ✅ Concluída. Nenhuma fase atende hoje a todos os critérios de pronto das suas etapas, por isso nenhuma está ✅. "Parcial" indica código que existe, mas não cobre os critérios de aceite do requisito.

| Fase | Sistema | Estado | Evidência (arquivo:linha) | Próxima ação |
| --- | --- | --- | --- | --- |
| S1 | Fundação visual (028, 029) | 🔨 Implementando | Etapa 1 concluída (PASSOU após a C1; F1 concluída, Plano Roadmap). Os componentes base de 029 (Etapa 2) não existem. Tokens em `desktop/src/styles/tokens.css`. | Planejar a Etapa 2 (029). A pasta dos componentes compartilhados é decisão aberta (seção 6). |
| S2 | Marca e identidades (030, 031, 032, 027) | ⏳ Pendente | Parcial em 030: a logo em SVG está em `desktop/src/shell/AppRail.vue:3-12` e os ícones em `desktop/src-tauri/icons/`. Não existem o mascote (031), as identidades (032) nem o retrato (027). | Decidir PEN-089 a PEN-091 antes da Etapa 3. |
| S3 | Trilho e uso dos planos (022, 023) | ✅ Concluída | Etapas 7, 7B, 7D, 8 e 8R concluídas no Plano Roadmap (a 7C foi substituída pela 7D). Ártemis PASSOU; Apolo APROVADO na 7D, na 8R e na revisão da 8. Trilho em `desktop/src/shell/AppRail.vue`; uso dos planos em `desktop/src/features/usage/`. | Nenhuma etapa aberta. Desvios do protótipo aceitos: D-L, D-M, D-N e D-O (Plano Roadmap). |
| S4 | Estrutura e estado da barra (001–005) | ⏳ Pendente | Parcial: uma pílula fixa (`desktop/src/features/team/ActionBar.vue:14-15`) com anel (:6-13) e placeholder (:20, :32). Não há compacta/expandida, ☰ nem mascote. Os estados vêm de `desktop/src/features/team/events.ts:7`, só `thinking`, `waiting` e `idle`. | Depende da Etapa 2. |
| S5 | Regras do balão (014) | ⏳ Pendente | Não há store nem componente de balão em `desktop/src/features/`. | Depende da Etapa 9. |
| S6 | Pedido (006–010) | 📝 Planejando | Parcial em 006/007: o @ foi validado em `desktop/e2e/RESULTADO-mencoes.md:1`, com `desktop/src/features/team/MentionMenu.vue:1` e `ActionBar.vue:3-4`. Não existem /, compositor nem triagem. | Depende das Etapas 6, 9 e 12. |
| S7 | Decisões e acompanhamento (011–013) | ⏸️ Aguardando decisão | `desktop/src/features/team/events.ts:31` só tem os eventos message, agent-state e error. A origem das decisões no backend não está no backlog (seção 6). | O usuário decide a origem das decisões. |
| S8 | Avisos do time (015, 016) | ⏳ Pendente | Não existe código. 016 depende da fonte de uso de 5 h (seção 6). | Depende das Etapas 8, 10 e 12. |
| S9 | Fluxo em etapas (017) | ⏳ Pendente | Não existe código. | Depende das Etapas 11 e 17. |
| S10 | Backlog e plano do item (024–026) | ⏳ Pendente | Não existe código. A persistência não está no backlog (seção 6). | Depende das Etapas 2 e 6. |
| S11 | Barra fora do app (018–021) | ⏳ Pendente | Parcial em 018: a bandeja tem só o menu Abrir/Sair (`desktop/src-tauri/src/tray.rs:16-34`). | Depende das Etapas 8, 10, 16 e 18. |
| S12 | Telas de sistema (033) | ⏳ Pendente | Parcial: a tela de CLI ausente está em `desktop/src/App.vue:26-33`, e a atualização em `desktop/src/features/updater/UpdatePrompt.vue`. Não existem splash nem tela de vazio. | Depende das Etapas 2 e 4. |

## Caminho até o trilho

### Fases que precisam estar concluídas antes, na ordem de execução

1. **Etapa 1 · Completar tokens de design (028).** O trilho só usa tokens: `--ink`, `--bg`, `--surface`, `--surface-2`, `--line`, `--muted`, `--accent`, `--accent-soft`, `--warn`, `--ok`, `--danger` e `--shadow-float`. Todos já existem em `tokens.css:6-27`. A etapa só precisa confirmar os critérios de 028 e acrescentar o que faltar para o trilho (ver Tokens abaixo).
2. **Etapa 7 · Trilho de ícones (022).** Depende só da Etapa 1. As Etapas 2 a 6 não são pré-requisito: o trilho do protótipo não usa componentes base, mascote nem retrato. A logo já existe, idêntica (`AppRail.vue:4-11`).
3. **Etapa 8 · Uso dos planos no trilho (023).** Depende da Etapa 7. Fecha a fidelidade do rodapé: avatar, anéis, ponto, linhas e painel.

Com isso, o trilho fica idêntico ao protótipo sem esperar S2 nem S4. Para a Etapa 8 ser fiel ao protótipo, os dados de uso precisam vir do mock (ver Pendências/Dúvidas).

### Fonte do protótipo

O componente "Trilho de ícones" é o mesmo em todas as 14 páginas de `docs/prototipo/` que o usam: o template comparado, sem os ids de recurso, é igual em todas. Fora do app.html e Onboarding.html não têm trilho. Está empacotado dentro de `docs/prototipo/Design.html`, no quadro "Trilho de ícones (componente)". Props do componente: `active` (projetos, agentes, estatisticas, skills, mapa, backlog, conversas, plano, canvas, diff, memoria, config; padrão `mapa`), `expanded` (padrão `false`), `uso` (normal, alerta, pausado; padrão `normal`) e `usoAberto` (padrão `false`). Prévia: 88 × 760.

### Telas e estados que precisam ser reproduzidos

- **Em um projeto** (`active` é uma ferramenta): bloco Workspace, depois o bloco "Projeto loja-online" com a entrada "Trocar de projeto: loja-online" (title "loja-online · trocar"), um divisor e as 7 ferramentas.
- **No workspace** (qualquer outro `active`, inclusive `config`): bloco Workspace, depois o bloco "Projetos" com 3 projetos, um divisor e "Novo projeto".
- **Recolhido / expandido:** 88 px ou 232 px de largura, com transição de largura `.18s ease`.
- **Item ativo / inativo:** ativo com fundo `--accent-soft` e cor `--accent`; inativo com cor `--muted`.
- **Uso normal / alerta / pausado:** anéis, ponto e painel.
- **Painel "Uso dos planos" fechado / aberto** (`role="dialog"`).

### Componentes e estrutura (literal)

- **Raiz:** coluna flex com `gap: 12px`, fonte Geist e `box-sizing: border-box`. Padding `20px 0` recolhido e `20px 16px` expandido. Alinhamento `center` recolhido e `stretch` expandido.
- **Linha da logo:** um link (`aria-label` "Vokia, início", title "Vokia") de 40 × 40, raio 12, fundo `--ink`, com o SVG da logo de 24 × 24 (viewBox 0 0 64 64, os 3 paths de `AppRail.vue:5-10`). Expandido, mostra "Vokia" ao lado (15 px, peso 600, `--ink`, `order: 2`) e a linha ganha `padding-left: 4px` e `gap: 10px`. Destino: Projetos.
- **Cartão de bloco** (os 3 `nav`): padding 5, `gap: 2px`, fundo `--surface`, borda 1 px `--line`, raio 14, sombra `--shadow-float`. Expandido, largura 100%.
- **Item recolhido:** 38 × 38, raio 10, grid centralizado. **Item expandido:** altura 36, raio 10, flex com `gap: 10px`, padding `0 10px`, `white-space: nowrap`. O ícone tem 17 × 17 (viewBox 0 0 16 16, traço 1.4, `round`/`round`). O rótulo tem 13 px, com elipse. `aria-label` e `title` levam o nome da página.
- **Selo de item** (`badge`): ponto de 7 px em `top: 6px; right: 6px`, `--warn`, com anel de 2 px `--surface`. Nenhum item do protótipo o ativa. O componente prevê o selo, mas nenhuma regra diz quando ele aparece (ver pendências).
- **Entrada "trocar de projeto":** recolhida tem 38 × 38; expandida tem altura 44, raio 10, fundo `--surface-2`, "loja-online" em 13 px peso 600 e "trocar de projeto" em 11 px `--muted`. Ícone de 16 × 16 (viewBox 0 0 18 18): um círculo r 6.5 com traço 1.5 e um círculo cheio r 2.5, ambos `--ink`.
- **Divisor:** 1 px `--line`, margem `3px 6px`.
- **Projeto da lista:** ícone de 16 × 16 (viewBox 0 0 18 18, traço 1.5). O ponto de estado tem 7 px e anel de 2 px `--surface`: recolhido fica em `bottom: 6px; right: 6px`; expandido fica estático, com `margin-left: auto`. Rótulo: o nome do projeto. `aria-label`/title: "<nome> · <n> trabalhando".
- **"Novo projeto":** ícone "+" (`M8 3v10M3 8h10`, traço 1.5). Destino: Onboarding.
- **Rodapé:** `margin-top: 8px`, coluna com `gap: 4px`. Contém "Configurações" (ícone de engrenagem, ativo quando `active=config`), o botão de alternar e o avatar.
- **Botão de alternar:** `aria-label`/title "Expandir menu" ou "Recolher menu", com `aria-expanded`. Ícone de painel (`M2 2.5h12v11H2zM6 2.5v11`) mais uma seta: `M9 6l2 2-2 2` recolhido e `M11 6L9 8l2 2` expandido. O texto visível expandido é **"Recolher"**. Sem borda, fundo transparente.
- **Avatar com anéis:** caixa de 44 × 44 com SVG 44 × 44. O trilho do anel externo tem r 20.5 e o do interno r 17, ambos com traço 2.4 em `--line`. O progresso usa `stroke-linecap: round`, `rotate(-90 22 22)` e `dasharray = (2πr · p/100).toFixed(1) + ' ' + (2πr).toFixed(1)`. O externo é o Claude; o interno, o GPT. O "K" fica em um círculo de 28 px, fundo `--surface-2`, 12 px peso 600, borda 1 px `--line`, title "Você". O ponto de alerta tem 9 px em `top: 1px; right: 1px`, anel de 2 px `--surface` e a cor da faixa do maior uso, e só aparece quando o maior uso é ≥ 75.
- **Linha do avatar:** `gap: 10px`, `margin-top: 4px`. Expandida, `padding: 2px 4px` e largura 100%. Expandido, mostra "Kay" (12.5 px, peso 500) e uma linha por plano ("Claude 5h", "GPT 5h"): nome com 40 px de largura, barra com altura 4 e raio 2 sobre `--line`, e porcentagem com 28 px, alinhada à direita e com `tabular-nums`. A porcentagem fica em negrito 600, na cor da faixa, quando ≥ 75. Linha em 11 px `--muted`, `gap: 6px`.
- **Botão do avatar:** `aria-label`/title "Uso dos planos: Claude <n>% e GPT <m>% da janela de 5 h", com `aria-expanded`. Clicar alterna o painel.
- **Painel "Uso dos planos":** posição absoluta em `left: calc(100% + 10px); bottom: 0`, z-index 40, largura 312, raio 16, padding 14, `gap: 12px`, borda `--line` e sombra `--shadow-float`. Cabeçalho: "Uso dos planos" (13.5 px, negrito), "atualizado agora" (11.5 px `--muted`) e o botão "Fechar" (12 px `--muted`, à direita, fecha o painel). Um bloco por plano:
  - Linha de 12.5 px com o nome (600), o plano (`--muted`) e o estado, à direita, em 11 px peso 600.
  - Linhas "5 h" e "7 d" em grid `26px minmax(0,1fr) 36px`, `gap: 8px`, 11.5 px. A barra tem altura 6, raio 3 e fundo `--surface-2`. A porcentagem é peso 500 com `tabular-nums`.
  - A nota "<renova> · <previsão>" em 11.5 px `--muted`.

  Rodapé do painel, com borda superior `--line` e `padding-top: 10px`: "Ver em Estatísticas" (flex 1, altura 32, raio 10, fundo `--ink`, texto `--bg`, 12.5 px, 600) e "Limites" (altura 32, raio 10, padding `0 11px`, borda `--line`).

### Regras de cor e estado (literal)

- **Faixa de uso:** ≥ 90 usa `--danger`; ≥ 75 usa `--warn`; abaixo disso usa `--ink`. Vale para anel, barra e ponto.
- **Estado do plano no painel:** ≥ 90 é "pausado" (`--danger`); ≥ 75 é "perto do limite" (`--warn`); abaixo disso é "ok" (`--muted`).
- **Barras e anéis:** limitados a 100%.
- **Uso desconhecido:** é erro explícito (`Sidebar: uso desconhecido "<valor>"`), sem fallback.
- **Bloco Projeto x Projetos:** o bloco Projeto aparece só quando `active` é uma das 7 ferramentas; caso contrário, aparece a lista de Projetos.

### Ícones (paths literais, viewBox 0 0 16 16, salvo indicação)

| Item | Rótulo | Path |
| --- | --- | --- |
| projetos | Projetos | `M2 2.5h5v5H2zM9 2.5h5v5H9zM2 9.5h5v4H2zM9 9.5h5v4H9z` |
| agentes | Agentes e camadas | `M6 7.5a2.5 2.5 0 100-5 2.5 2.5 0 000 5zM1.5 14c0-2.6 2-4.2 4.5-4.2s4.5 1.6 4.5 4.2M11 3a2.2 2.2 0 010 4.3M12.3 9.9c1.4.5 2.2 1.9 2.2 4.1` |
| estatisticas | Estatísticas | `M2 13.5h12M4 11V8M7 11V4.5M10 11V6.5M13 11V3` |
| skills | Skills | `M9 1.5L3.5 9h4L6.5 14.5 12.5 6.5h-4z` |
| mapa | Mapa do time | `M1.5 3.5h13v9h-13zM1.5 6.5h13M5.5 3.5v9M10 3.5v9` |
| backlog | Backlog e planos | `M2 2.5h3.5v11H2zM6.25 2.5h3.5v7h-3.5zM10.5 2.5H14v4.5h-3.5z` |
| conversas | Conversas | `M1.5 3h8.5v6H5l-3.5 2.5zM12 6h2.5v6.5L12 11H7.5V9` |
| plano | Plano | `M3 3h2M3 8h2M3 13h2M7.5 3h6M7.5 8h6M7.5 13h6` |
| canvas | Artefatos | `M2 2.5h5v4H2zM9.5 9.5a2.5 2.5 0 105 0 2.5 2.5 0 00-5 0zM2 13.5l3-4 3 4zM7 4.5h3.5v3` |
| diff | Diff e revisão | `M4 2v8M4 12.5a1.5 1.5 0 100-.1M12 3.5a1.5 1.5 0 100-.1M12 5v2c0 2-2 3-5.5 3.5` |
| memoria | Memória e grafo | `M4 4a2 2 0 100-.1M12 4a2 2 0 100-.1M8 12a2 2 0 100-.1M5.5 5.5L7 10.5M10.5 5.5L9 10.5M6 4h4` |
| config | Configurações | círculo `cx 8 cy 8 r 2.2` + `M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4` |
| loja-online (viewBox 0 0 18 18) | projeto | `M9 2.5a6.5 6.5 0 110 13 6.5 6.5 0 010-13zM9 7a2 2 0 110 4 2 2 0 010-4z` |
| app-agenda (viewBox 0 0 18 18) | projeto | `M9 3l6.5 12h-13z` |
| site-estudio (viewBox 0 0 18 18) | projeto | `M3.5 3.5h11v11h-11z` |

### Tokens, tipografia e espaçamento

- **Cores:** todas já existem em `tokens.css:6-27`. Os fallbacks do protótipo batem com o tema claro atual: `--ink` #1c1c21, `--bg` #fafafa, `--surface` #fff, `--surface-2` #f3f3f5, `--line` #e6e6ea, `--muted` #6b6b76, `--accent` #5b57d1, `--accent-soft` #eeedfb, `--warn` #e07b24, `--ok` #2f9e5b, `--danger` #cf4a3c.
- **Raios:** 10 = `control` e 12 = `control-lg` (`tokens.css:34-35`); 14 = `card` e 16 = `popover` (:36-37). Os raios 2 e 3 das barras de uso não têm token.
- **Tamanhos de texto:** 15 (`lead`), 13.5 (`control`), 13 (`label`), 12.5 (`caption`), 12 (`small`), 11.5 (`meta`) e 11 (`tiny`) já existem (`tokens.css:48-64`). Os pesos 500 e 600 são usados de forma literal.
- **Medidas sem token, a reproduzir como constantes nomeadas:**
  - Larguras do trilho: 88 e 232.
  - Itens: 38 e 36; entrada de projeto: 44.
  - Logo: 40; avatar: 44 e 28.
  - Anéis: r 20.5 e 17, traço 2.4.
  - Pontos: 7 e 9.
  - Painel: 312, deslocamento 10.
  - Ícones: 17 e 16.
  - Transição: `.18s ease`.

### Dados mockados (literais do protótipo)

- **Projetos:** loja-online · 5 trabalhando (ponto `--warn`); app-agenda · 2 trabalhando (`--warn`); site-estudio · 2 trabalhando (`--ok`).
- **Usuário:** avatar "K", nome "Kay".
- **Planos:** "Claude Code" com "plano Max" (rótulo curto "Claude") e "Codex" com "ChatGPT Plus" (rótulo curto "GPT").
- **Uso** (5 h, 7 d, renovação, previsão):

| Cenário | Claude | GPT |
| --- | --- | --- |
| normal | 62%, 41%, "renova em 1 h 40", "no ritmo atual, a janela acaba em ~2 h" | 28%, 19%, "renova em 3 h 10", "folga para o resto do dia" |
| alerta | 78%, 52%, "renova em 1 h 05", "no ritmo atual, acaba em ~25 min" | 31%, 20%, "renova em 3 h 10", "folga para o resto do dia" |
| pausado | 91%, 58%, "renova em 38 min", "time pausado em 90% · volta sozinho" | 34%, 21%, "renova em 2 h 50", "Codex segue disponível" |

### O que não pode ser simplificado

- Os dois modos (88/232) com o mesmo conteúdo e o toggle no rodapé, com `aria-expanded`.
- A troca entre o bloco Projeto e a lista de Projetos pela página ativa.
- Os 7 itens de projeto e os 4 do Workspace, na ordem e com os paths da tabela. Não trocar por uma biblioteca de ícones.
- Os dois anéis concêntricos com dasharray calculado, o ponto de alerta e as três faixas de cor.
- O painel com as linhas "5 h" e "7 d" por plano, as notas, "atualizado agora", "Fechar", "Ver em Estatísticas" e "Limites".
- Todos os `aria-label` e `title` literais.
- O erro explícito para uso desconhecido.

### Pendências/Dúvidas do trilho

- **Fonte dos dados:** usar o mock acima no app real ou ler da CLI? O uso de 5 h/7 d depende do que a CLI expõe (visão de produto, seção 13). A lista de projetos e o "Kay" também não têm fonte no código.
- **Destinos:** as páginas de destino (Projetos, Agentes, Estatísticas, Skills, Mapa e as demais) não existem no app. O que cada item faz ao clicar? O protótipo aponta para `*.dc.html`; "Configurações" e "Limites" apontam para `Gastos.dc.html` (PEN-058, PEN-059).
- **Persistência:** o modo expandido fica "salvo por pessoa" (PEN-057), mas o protótipo guarda só em memória.
- **Página atual:** hoje o app só tem Conversas. A página ativa deve ser `conversas` (bloco Projeto)?
- **Selo `badge`:** o componente o prevê, mas nenhum item o usa e nenhuma regra diz quando aparece.
- **Tema escuro:** o protótipo só define os fallbacks claros. Basta usar os tokens com `light-dark()`?
- **Fonte Geist:** o protótipo usa Geist estática (400/500/600) e o app usa Geist Variable (`main.css:1`). A troca de família por escala é a PEN-084.

## 1. Visão curta

A Vokia é um app desktop local (Tauri 2 com Vue 3 e Rust) que orquestra agentes do Claude Code. O MVP já envia pedidos à Iara, que delega ao Davi. Falta entregar os 33 requisitos do backlog: o sistema visual e a marca, a barra de ação completa (estado, pedido, decisões, balões e fluxos), o trilho com o uso dos planos, os componentes do backlog, a barra fora do app e as telas de sistema.

### O que já existe no código (não replanejar, só completar)

| Requisito | O que existe | Onde |
| --- | --- | --- |
| 028 Tokens e tema | Tokens claro/escuro com `light-dark()` e seletor Claro/Escuro/Sistema, validados | `desktop/src/styles/tokens.css`, `desktop/src/features/theme/`, `desktop/e2e/RESULTADO-tokens.md` |
| 001/002/006 Barra | Uma pílula fixa embaixo, com anel de estado, campo e placeholder "Peça algo ao time" | `desktop/src/features/team/ActionBar.vue` |
| 007 Menção com @ | Lista, filtro por prefixo, setas, Enter/Esc e chip do agente, validados | `desktop/src/features/team/MentionMenu.vue`, `desktop/e2e/RESULTADO-mencoes.md` |
| 022 Trilho | Trilho com a logo e o item Conversas | `desktop/src/shell/AppRail.vue` |
| 030 Ícone do app | Ícones do executável e logo em SVG no trilho | `desktop/src-tauri/icons/`, `desktop/app-icon.svg` |
| 018 Bandeja | Ícone com menu Abrir/Sair; o X esconde a janela | `desktop/src-tauri/src/tray.rs` |
| 033 CLI ausente | Tela com o motivo e o comando de correção (`cli_problem`) | `desktop/src-tauri/src/commands.rs`, `desktop/src/App.vue` |
| Estados dos agentes | `thinking`, `waiting` e `idle` por evento | `desktop/src/features/team/events.ts`, `desktop/src-tauri/src/events.rs` |

## 2. Ordem de sistemas

1. **S1 · Fundação visual (028, 029).** Todos os outros requisitos usam os tokens e os componentes base, e 028 não tem dependências. Vem primeiro.
2. **S2 · Marca e identidades (030, 031, 032, 027).** O mascote (004), o @ (007), o painel Acontecendo agora (013) e os cards (024) usam a chama e o retrato. Depende de S1 (tokens).
3. **S3 · Trilho e uso dos planos (022, 023).** O limite do plano na barra (016) e a bandeja (018) leem o uso de 5 h exposto aqui. Depende de S1.
4. **S4 · Estrutura e estado da barra (001, 002, 003, 004, 005).** É a moldura onde entram o pedido, as decisões, os balões e os fluxos. Depende de S1 e S2 (o mascote em 004).
5. **S5 · Regras do balão (014).** A triagem (010), os momentos (015) e os limites (016) são balões e seguem estas regras. Depende de S4.
6. **S6 · Pedido (006, 007, 008, 009, 010).** Depende do campo na barra expandida (S4) e do balão (S5) para a triagem.
7. **S7 · Decisões e acompanhamento (011, 012, 013).** O estado "Precisa de você" (003), o contador "3" e a barra fora do app dependem do painel de decisões. Depende de S4 e S6 (o pedido gera trabalho para acompanhar).
8. **S8 · Avisos do time (015, 016).** São balões (S5) que usam o mascote (004) e o uso dos planos (S3).
9. **S9 · Fluxo em etapas (017).** Substitui o ☰ e o botão de decisões, então depende de S4 e S7.
10. **S10 · Backlog e plano do item (024, 025, 026).** São componentes de tela que usam S1 e o retrato (S2). Vêm depois da barra porque a visão de produto põe a barra em todas as telas e o kanban na fase 1, depois da base.
11. **S11 · Barra fora do app (018, 019, 020, 021).** Repete o estado (003), as decisões (011/012) e os limites (023) fora da janela. Depende de S3, S4 e S7.
12. **S12 · Telas de sistema (033).** Usa o mascote (031) e o bloco "Vazio" dos componentes base (029). Pode avançar assim que S1 e S2 estiverem prontos. Fica no fim porque a visão de produto a põe na fase de acabamento.

## 3. Execuções

Convenções para todas as etapas:

- **Nota da etapa:** `Etapa N - <título>` no fichário `plan`. O Planejador cria a nota quando o Maestro pede o plano da etapa.
- **Terminal:** o Maestro preenche ao distribuir. "a definir" é o placeholder.
- **Verificação do contrato** (README, seção Verificações), nesta ordem: `cd desktop/src-tauri && cargo fmt --check && cargo clippy -- -D warnings` (só se mudou Rust), depois `cd desktop && npm run typecheck && npm run build`.
- **E2E primeiro:** um roteiro na janela de `npm run tauri dev`, com o WebView2 em modo depuração (CDP na porta 9222), seguindo o padrão de `desktop/e2e/mencoes.cdp.mjs`. O resultado vai para `desktop/e2e/RESULTADO-<tema>.md`.
- **Camadas:** componente → store (`useX.ts`) → serviço (`xApi.ts`) → Tauri. Só os serviços importam `@tauri-apps`. Tokens só em `desktop/src/styles/tokens.css`.
- **Não fazer em nenhuma etapa:** implementar comportamento que depende de uma PEN aberta. Nesse caso a etapa para e responde BLOQUEADO.

### S1 · Fundação visual

| Etapa | Nota da etapa | Papel | Terminal | Nível | Dependências | Estado |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Etapa 1 - Completar tokens de design (028) | Implementador | a definir | Intermediário | — | ⏳ Pendente |
| 2 | Etapa 2 - Componentes base (029) | Implementador | a definir | Intermediário | 1 | ⏳ Pendente |

- **Etapa 1.** *Objetivo:* completar tipografia, medidas e ícones de 028 sobre os tokens existentes. *Arquivos:* `desktop/src/styles/tokens.css`, `desktop/src/styles/main.css`. *Pronto:* os critérios de aceite de `backlog/028.md` são atendidos nos dois temas. *Verificação:* E2E visual nos temas Claro, Escuro e Sistema, como em `RESULTADO-tokens.md`, mais o contrato. *Não fazer:* trocar o tema de lugar (PEN-083) nem escolher fonte por escala (PEN-084).
- **Etapa 2.** *Objetivo:* criar botões, campos, selos, cartões, código, comando e avisos de retorno. *Arquivos:* nova pasta de componentes compartilhados, a decidir no plano (ver seção 6). *Pronto:* os critérios de `backlog/029.md`. *Verificação:* E2E de uso real em uma tela, mais o contrato. *Não fazer:* Desfazer, Esquecer nem estados de interação sem decisão (PEN-086, PEN-087, PEN-088).

### S2 · Marca e identidades

| Etapa | Nota da etapa | Papel | Terminal | Nível | Dependências | Estado |
| --- | --- | --- | --- | --- | --- | --- |
| 3 | Etapa 3 - Logo e ícone do app (030) | Implementador | a definir | Básico | 1 | ⏳ Pendente |
| 4 | Etapa 4 - Mascote da Vokia (031) | Implementador | a definir | Intermediário | 3 | ⏳ Pendente |
| 5 | Etapa 5 - Biblioteca de identidades visuais (032) | Implementador | a definir | Intermediário | 1 | ⏳ Pendente |
| 6 | Etapa 6 - Retrato de agente (027) | Implementador | a definir | Intermediário | 5 | ⏳ Pendente |

- **Etapa 3.** *Objetivo:* logo e variações do ícone (executável, dock, bandeja). *Arquivos:* `desktop/app-icon.svg`, `desktop/src-tauri/icons/`, `desktop/src/shell/AppRail.vue`. *Pronto:* `backlog/030.md`. *Verificação:* build do instalador e o ícone visto na bandeja e no trilho. *Não fazer:* escolher a variação padrão (PEN-089 a PEN-091).
- **Etapa 4.** *Objetivo:* a chama com poses e estados. *Arquivos:* um novo componente de mascote em `desktop/src/features/`. *Pronto:* `backlog/031.md`. *Verificação:* E2E com cada pose renderizada. *Não fazer:* usar o mascote na barra (isso é da Etapa 10).
- **Etapa 5.** *Objetivo:* avatares, emblemas e símbolos, com marca de estado além da cor. *Pronto:* `backlog/032.md`. *Não fazer:* a família por conceito nem a ação "tirar" (PEN-092, PEN-093).
- **Etapa 6.** *Objetivo:* um retrato quadrado único em todos os tamanhos, aplicado em `MentionMenu.vue` e `AgentList.vue`. *Pronto:* `backlog/027.md`. *Não fazer:* marcas de estado, papéis divergentes nem o crachá (PEN-078 a PEN-082).

### S3 · Trilho e uso dos planos

| Etapa | Nota da etapa | Papel | Terminal | Nível | Dependências | Estado |
| --- | --- | --- | --- | --- | --- | --- |
| 7 | Etapa 7 - Trilho de ícones (022) | Implementador | a definir | Intermediário | 1 | ⏳ Pendente |
| 8 | Etapa 8 - Uso dos planos no trilho (023) | Implementador | a definir | Intermediário | 7 | ⏳ Pendente |

- **Etapa 7.** *Objetivo:* páginas e ferramentas, com o trilho recolhido ou expandido. *Arquivos:* `desktop/src/shell/AppRail.vue`, `desktop/src/App.vue`. *Pronto:* `backlog/022.md`. *Verificação:* E2E alternando os modos. *Não fazer:* persistir o modo nem dar destino a Configurações (PEN-057, PEN-058).
- **Etapa 8.** *Objetivo:* anéis e painel com o uso da janela de 5 h por plano. *Arquivos:* trilho, uma nova feature de uso (store e serviço) e possivelmente Rust. *Pronto:* `backlog/023.md`. *Não fazer:* dar destino a "Limites" (PEN-059). A fonte do dado de uso é risco (seção 6).

### S4 · Estrutura e estado da barra

| Etapa | Nota da etapa | Papel | Terminal | Nível | Dependências | Estado |
| --- | --- | --- | --- | --- | --- | --- |
| 9 | Etapa 9 - Anatomia, compacta e expandida (001, 002) | Implementador | a definir | Intermediário | 2 | ⏳ Pendente |
| 10 | Etapa 10 - Estado do time e mascote na barra (003, 004) | Implementador | a definir | Intermediário | 4, 9 | ⏳ Pendente |
| 11 | Etapa 11 - Menu de ações por página (005) | Implementador | a definir | Intermediário | 9 | ⏳ Pendente |

- **Etapa 9.** *Objetivo:* as quatro partes em uma linha, compacta por padrão, expandida com o mouse ou o foco. *Arquivos:* `desktop/src/features/team/ActionBar.vue` e um possível desmembramento em subcomponentes. *Pronto:* `backlog/001.md` e `backlog/002.md`. *Verificação:* E2E por hover e foco. *Não fazer:* rolagem, onboarding, modo configurável, ✓ do contador nem a volta sem hover (PEN-016 a PEN-020).
- **Etapa 10.** *Objetivo:* um estado por vez, pela ordem Pausado > Erro > Precisa de você > Trabalhando > Repouso, com o mascote só em mensagens gerais e no limite. *Arquivos:* `useTeam.ts`, `events.ts`, `events.rs`, `ActionBar.vue`. *Pronto:* `backlog/003.md` e `004.md`. *Não fazer:* posicionar Em paralelo, Concluído e Pensando na ordem nem definir a duração de Concluído (PEN-001, PEN-002, PEN-012, PEN-021, PEN-011).
- **Etapa 11.** *Objetivo:* o ☰ com as ações da página e os atalhos ⌥n. *Pronto:* `backlog/005.md`. *Não fazer:* ações que não abrem pedido (PEN-004).

### S5 · Regras do balão

| Etapa | Nota da etapa | Papel | Terminal | Nível | Dependências | Estado |
| --- | --- | --- | --- | --- | --- | --- |
| 12 | Etapa 12 - Balão de aviso: regras gerais (014) | Implementador | a definir | Intermediário | 9 | ⏳ Pendente |

- **Etapa 12.** *Objetivo:* um balão por vez, timer de 8 s, origem visível, Esc/⌘J, e fechar não bloqueia os próximos. *Arquivos:* uma nova store e um novo componente de balão em `features/team/` ou em uma feature própria. *Pronto:* `backlog/014.md` e as decisões aceitas. *Não fazer:* anatomia, tela que cala, "Próximo momento" nem fila fora do diagrama (PEN-043 a PEN-046).

### S6 · Pedido

| Etapa | Nota da etapa | Papel | Terminal | Nível | Dependências | Estado |
| --- | --- | --- | --- | --- | --- | --- |
| 13 | Etapa 13 - Campo de pedido e @ (006, 007) | Implementador | a definir | Intermediário | 9, 6 | ⏳ Pendente |
| 14 | Etapa 14 - Comandos com / (008) | Implementador | a definir | Intermediário | 13 | ⏳ Pendente |
| 15 | Etapa 15 - Compositor de novo pedido (009) | Implementador | a definir | Intermediário | 11, 13 | ⏳ Pendente |
| 16 | Etapa 16 - Triagem do pedido no balão (010) | Implementador | a definir | Intermediário | 12, 15 | ⏳ Pendente |

- **Etapa 13.** *Objetivo:* completar o campo e o @ que já existem, com placeholder por estado e retrato na lista. *Pronto:* `backlog/006.md` e `007.md`, sem regredir `RESULTADO-mencoes.md`. *Não fazer:* PEN-008, PEN-022 a PEN-026.
- **Etapa 14.** *Objetivo:* a lista de comandos ao digitar /. *Não fazer:* inventar comandos (PEN-006, PEN-027, PEN-028).
- **Etapa 15.** *Objetivo:* um modal com a descrição, o tamanho e o agente. *Não fazer:* o efeito de Cancelar nem o envio sem decisão (PEN-005, PEN-029, PEN-030).
- **Etapa 16.** *Objetivo:* o balão com a etapa do pedido, da escrita à aprovação. A Iara classifica e o padrão é confirmar com o balão. *Arquivos:* `orchestrator.rs`, a store do balão, `ActionBar.vue`. *Não fazer:* o modo automático nem os textos e estados sem decisão (PEN-010, PEN-031 a PEN-033, PEN-007).

### S7 · Decisões e acompanhamento

| Etapa | Nota da etapa | Papel | Terminal | Nível | Dependências | Estado |
| --- | --- | --- | --- | --- | --- | --- |
| 17 | Etapa 17 - Painel de decisões (011) | Implementador | a definir | Intermediário | 10 | ⏳ Pendente |
| 18 | Etapa 18 - Decisão em linha (012) | Implementador | a definir | Intermediário | 17 | ⏳ Pendente |
| 19 | Etapa 19 - Painel Acontecendo agora (013) | Implementador | a definir | Intermediário | 17, 13 | ⏳ Pendente |

- **Etapa 17.** *Objetivo:* um painel com as decisões pendentes e o contador. Entram só as decisões com a opção "Outro…". *Não fazer:* PEN-034 a PEN-036, PEN-096. A origem das decisões no backend é pendência (seção 6).
- **Etapa 18.** *Objetivo:* na linha da barra, só permissão de comando e pergunta com até duas opções curtas. *Não fazer:* PEN-015, PEN-037 a PEN-039.
- **Etapa 19.** *Objetivo:* tarefa, etapas, últimas ações, fila e números do dia por agente. *Não fazer:* PEN-040 a PEN-042.

### S8 · Avisos do time

| Etapa | Nota da etapa | Papel | Terminal | Nível | Dependências | Estado |
| --- | --- | --- | --- | --- | --- | --- |
| 20 | Etapa 20 - Momentos do time (015) | Implementador | a definir | Intermediário | 12, 10 | ⏳ Pendente |
| 21 | Etapa 21 - Limites do plano na barra (016) | Implementador | a definir | Intermediário | 12, 10, 8 | ⏳ Pendente |

- **Etapa 20.** Momento com o painel aberto espera na fila. *Não fazer:* PEN-047.
- **Etapa 21.** Avisa em 75%, pausa em 90% e mostra a renovação, com a pose correspondente do mascote. *Não fazer:* PEN-048, PEN-049.

### S9 · Fluxo em etapas

| Etapa | Nota da etapa | Papel | Terminal | Nível | Dependências | Estado |
| --- | --- | --- | --- | --- | --- | --- |
| 22 | Etapa 22 - Fluxo em etapas na barra (017) | Implementador | a definir | Intermediário | 11, 17 | ⏳ Pendente |

- **Etapa 22.** O botão "Pronto" na última etapa encerra o fluxo. *Não fazer:* telas de destino, mudança de etapa nem o painel ao entrar no fluxo (PEN-050, PEN-051, PEN-013).

### S10 · Backlog e plano do item

| Etapa | Nota da etapa | Papel | Terminal | Nível | Dependências | Estado |
| --- | --- | --- | --- | --- | --- | --- |
| 23 | Etapa 23 - Card do backlog (024) | Implementador | a definir | Intermediário | 2, 6 | ⏳ Pendente |
| 24 | Etapa 24 - Modal do item do backlog (025) | Implementador | a definir | Intermediário | 23 | ⏳ Pendente |
| 25 | Etapa 25 - Plano do item (026) | Implementador | a definir | Intermediário | 24 | ⏳ Pendente |

- *Não fazer:* PEN-060 a PEN-077. A persistência dos itens (SQLite, visão 6.4) não está no backlog (seção 6).

### S11 · Barra fora do app

| Etapa | Nota da etapa | Papel | Terminal | Nível | Dependências | Estado |
| --- | --- | --- | --- | --- | --- | --- |
| 26 | Etapa 26 - Ícone na bandeja (018) | Implementador | a definir | Intermediário | 10, 18, 8 | ⏳ Pendente |
| 27 | Etapa 27 - Notificação fora do app (021) | Implementador | a definir | Intermediário | 16, 18 | ⏳ Pendente |
| 28 | Etapa 28 - Widget na barra de tarefas (019) | Implementador | a definir | Intermediário | 27 | ⏳ Pendente |
| 29 | Etapa 29 - Lateral da tela (020) | Implementador | a definir | Intermediário | 27 | ⏳ Pendente |

- *Arquivos prováveis:* `desktop/src-tauri/src/tray.rs`, `window.rs`, `main.rs` e novas janelas. Fora do app só se aprovam comandos. *Verificação:* `cargo fmt`/`clippy` mais o E2E com a janela fora de foco. *Não fazer:* escolher o modo (PEN-009), a legenda (PEN-052) nem o gatilho, a natureza e a duração do aviso (PEN-053 a PEN-056).

### S12 · Telas de sistema

| Etapa | Nota da etapa | Papel | Terminal | Nível | Dependências | Estado |
| --- | --- | --- | --- | --- | --- | --- |
| 30 | Etapa 30 - Splash, atualização, vazio e CLI ausente (033) | Implementador | a definir | Intermediário | 2, 4 | ⏳ Pendente |

- **Etapa 30.** Completa a tela de CLI ausente que já existe e reaproveita `features/updater/UpdatePrompt.vue`. *Verificação:* E2E sem CLI no PATH, como em `RESULTADO.md` item 5. *Não fazer:* PEN-094, PEN-095.

Cada etapa de implementação gera, na sequência, um pedido ao Tester (E2E) e outro ao Reviewer, com os prompts da seção 5.

## 4. Paralelismo

Podem rodar ao mesmo tempo, porque tocam arquivos diferentes:

- Etapas 3, 5 e 7, depois da Etapa 1: marca, identidades e trilho.
- Etapa 4 (mascote) junto com a 6 (retrato).
- Etapa 8 (uso dos planos) junto com S4 (Etapas 9 a 11), desde que a 8 não edite `ActionBar.vue`.
- Etapas 14 e 15, depois da 13, se o compositor ficar em um arquivo próprio.
- Etapas 18 e 19, depois da 17.
- Etapas 20 e 21.
- Etapas 23 a 25 (S10), em paralelo com S7 a S9: não tocam a barra.
- Etapa 30, em paralelo com qualquer etapa depois da 4.
- Etapas 28 e 29, depois da 27.

São sequenciais, porque tocam o mesmo arquivo ou dependem do estado:

- 1 → 2 → 9 → 10 → 11: a cadeia de `tokens.css` e `ActionBar.vue`.
- 12 → 16 e 12 → 20/21: a store do balão.
- 17 → 22: o fluxo substitui o botão de decisões.
- 26 e 27 sobre `tray.rs` e `window.rs`: uma de cada vez.
- O Tester e o Reviewer de uma etapa rodam depois do PRONTO do Implementador.

## 5. Prompts AI-made

Substitua os campos `<...>`. Os nomes das notas devem ser os da tabela.

### (a) Plano de uma etapa · Planejador

```text
Você é o Planejador. Não vê a conversa do usuário; este é o pedido completo.
OBJETIVO: planejar a <Etapa N - título> do roadmap (docs/roadmap.md, seção 3), que cobre o(s) requisito(s) <IDs> de docs/backlog/.
DIRETÓRIO: <raiz do repositório Vokia>.
ARQUIVOS: leia docs/backlog/<ID>.md, as PEN citadas, as Decisões aceitas em docs/PENDENCIAS.md e, por trecho, os arquivos prováveis da etapa: <lista>.
NOTA DE PLANO: '<Plano X>' (crie em --stack plan se não existir). Notas de etapa no fichário plan, com o nome '<Etapa N - título>'.
CRITÉRIO DE PRONTO: nota de etapa com passos, arquivos, reutilização (caminho:linha), âncoras, mudança exata, formas de falha para o Tester, verificação na ordem do contrato e nível sugerido; nenhuma decisão para o implementador. Comportamento ligado a PEN aberta vai para Perguntas em aberto.
Não implemente, não crie testes, não faça git.
NOTA DE STATUS: 'Status Terminais' (não leia nem escreva; o Maestro grava).
RESPOSTA (máx. 15 linhas): primeira linha ESTADO: PRONTO|BLOQUEADO; depois Feito, Arquivos, Verificação, Pendências/Dúvidas; linha 'Estado: <emoji> <estado>' e até 3 linhas em 'Detalhe:'.
```

### (b) Implementação · Implementador

```text
Você é o Implementador. Não vê a conversa do usuário; este é o pedido completo.
OBJETIVO: implementar exatamente a nota '<Etapa N - título>' da nota de plano '<Plano X>'. Leia as duas com maestri note read; elas são a especificação. Não siga resumo deste pedido no lugar delas.
DIRETÓRIO: <raiz do repositório Vokia>.
ARQUIVOS: só os listados na nota da etapa. Precisar de outro arquivo = BLOQUEADO.
CRITÉRIO DE PRONTO: o critério de aceite da nota da etapa, e os comandos de verificação da nota passando na ordem do contrato (cargo fmt --check e cargo clippy -- -D warnings se houver Rust; npm run typecheck e npm run build em desktop).
Âncora divergente, valor de regra ausente ou falha de ambiente = BLOQUEADO. Não crie testes. Não faça commit.
NOTA DE STATUS: 'Status Terminais' (não leia nem escreva; o Maestro grava).
RESPOSTA (máx. 15 linhas): primeira linha ESTADO: PRONTO|BLOQUEADO; depois Feito, Arquivos (arquivo:linha), Verificação (saída exata das falhas), Pendências/Dúvidas; linha 'Estado: <emoji> <estado>' e até 3 linhas em 'Detalhe:'.
```

### (c) Testes E2E · Tester

```text
Você é o Tester. Não vê a conversa do usuário; este é o pedido completo.
OBJETIVO: validar por E2E a '<Etapa N - título>' da nota de plano '<Plano X>', cobrindo as "Formas de falha para o Tester" da nota da etapa.
DIRETÓRIO: <raiz do repositório Vokia>.
ARQUIVOS: roteiro em desktop/e2e/<tema>.cdp.mjs, no padrão de desktop/e2e/mencoes.cdp.mjs (npm run tauri dev, WebView2 com CDP na porta 9222); resultado em desktop/e2e/RESULTADO-<tema>.md. Não altere código de produção.
CRITÉRIO DE PRONTO: cada forma de falha com um cenário real e evidência; contrato rodado (cargo fmt --check, cargo clippy -- -D warnings, npm run typecheck, npm run build); artefato repetível. Sem testes de detalhe visual.
NOTA DE STATUS: 'Status Terminais' (não leia nem escreva; o Maestro grava).
RESPOSTA (máx. 15 linhas): primeira linha ESTADO: PASSOU|FALHOU|BLOQUEADO; depois Feito, Arquivos, Verificação (até 10 linhas de erro), Pendências/Dúvidas; linha 'Estado: <emoji> <estado>' e até 3 linhas em 'Detalhe:'.
```

### (d) Revisão · Reviewer

```text
Você é o Reviewer. Não vê a conversa do usuário; este é o pedido completo.
OBJETIVO: revisar a implementação da '<Etapa N - título>' contra a nota da etapa e a nota de plano '<Plano X>', e contra docs/backlog/<ID>.md.
DIRETÓRIO: <raiz do repositório Vokia>.
ARQUIVOS: os alterados na etapa (<lista>) e o resultado do Tester em desktop/e2e/RESULTADO-<tema>.md.
CRITÉRIO DE PRONTO: conferir camadas (componente → store → serviço → Tauri; @tauri-apps só em serviços), tokens só em tokens.css, ausência de duplicação, código morto, números mágicos, comportamento de PEN aberta e escopo fora da nota; contrato verde.
Não altere arquivos. Não faça git.
NOTA DE STATUS: 'Status Terminais' (não leia nem escreva; o Maestro grava).
RESPOSTA (máx. 25 linhas): primeira linha ESTADO: APROVADO|REPROVADO|BLOQUEADO; depois Feito, Arquivos (arquivo:linha de cada achado), Verificação, Pendências/Dúvidas; linha 'Estado: <emoji> <estado>' e até 3 linhas em 'Detalhe:'.
```

## 6. Pendências e decisões em aberto

- **PEN-001 a PEN-096 abertas** (índice em [PENDENCIAS.md](PENDENCIAS.md)). Cada etapa lista as suas em "Não fazer". Antes de planejar uma etapa, o usuário decide as PEN dela. As que mais travam a ordem:
  - PEN-002, PEN-012 e PEN-001: a ordem completa de estados (Etapa 10).
  - PEN-005 e PEN-010: o envio do compositor e a triagem (Etapas 15 e 16).
  - PEN-015: a decisão em linha (Etapa 18).
  - PEN-053 a PEN-056: o aviso fora do app (Etapa 27).
- **Fora do backlog, mas necessário** (não planejado; decisão do usuário):
  - A origem das decisões no backend: permissões e perguntas dos agentes chegando ao painel 011. Hoje `events.rs` só tem message, agent-state e error.
  - A fonte do uso de 5 h por plano (023/016): a visão de produto, seção 13, diz que depende do que a CLI expõe e precisa ser validada.
  - A persistência (SQLite, visão 6.4) para o backlog, as decisões e as filas.
  - As páginas de destino do trilho e do ☰ (Projetos, Backlog, Configurações e outras), citadas nas PEN mas sem requisito próprio.
  - A pasta dos componentes base compartilhados (029): o README só prevê `features/<nome>/` e `shell/`.
  - A ordem relativa de S10 e S12 diante da barra: a visão de produto a sugere, mas o backlog não define prioridade.

## 7. Fontes

- `docs/BACKLOG.md` e `docs/backlog/001.md` a `docs/backlog/033.md`: requisitos e dependências.
- `docs/PENDENCIAS.md` e `docs/pendencias/PEN-001.md` a `PEN-096.md`: pendências e decisões aceitas.
- `docs/vokia-visao-de-produto.md`: seções 6.4, 6.5, 12 e 13, para ordenar e levantar riscos.
- Código conferido por trecho: `README.md`, `desktop/package.json`, `desktop/src/features/team/ActionBar.vue`, `MentionMenu.vue`, `events.ts`, `desktop/src/styles/tokens.css`, `desktop/src/shell/AppRail.vue`, `desktop/src-tauri/src/tray.rs` e `desktop/e2e/RESULTADO*.md`.
