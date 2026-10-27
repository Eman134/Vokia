# Resultado da validação: PLAN Menções e identificação de agentes

Data: 2026-10-26
Escopo: Etapas 1 a 4 do plano, com os cenários 1 a 8 do pedido.
Execução: `e2e/mencoes.cdp.mjs` na janela Vokia do `npm run tauri dev`, com o WebView2 em modo depuração (porta 9222). Os envios foram capturados no IPC (`http://ipc.localhost/send_request`) pelo domínio Fetch do CDP. Não houve alteração de código.
Evidências: `resultados.json` e capturas de tela na pasta de scratchpad da sessão.

## 1. Comandos

| Comando | Resultado |
|---|---|
| `cargo fmt --check` (src-tauri) | PASSOU |
| `cargo clippy -- -D warnings` (src-tauri) | PASSOU |
| `npm run typecheck` (desktop) | PASSOU |
| `npm run build` (desktop) | PASSOU |

## 2. Cenários

| Cenário | Estado | Evidência |
|---|---|---|
| 1. @ no início abre a lista; filtro por prefixo sem diferenciar maiúsculas; setas; Enter escolhe; Esc fecha; Nenhum agente | PASSOU | checks 1 (12 itens) |
| 1. Clique no agente da lista cria o chip | PASSOU | check "clique no agente cria chip e limpa o @" |
| 2. Enter com a lista aberta não envia (com "@x" e com "@d") | PASSOU | feed sem bolhas novas; texto "@x" mantido |
| 3. @ no meio do texto é texto comum | PASSOU | "oi @d" sem lista |
| 4. Escolher Davi cria chip "Davi ×" e placeholder "Mensagem para Davi" | PASSOU | snapshot do campo |
| 4. Envio vai ao Davi: IPC `to=davi`, texto sem menção; Davi pensa e Iara não | PASSOU | IPC capturado; estados "Davi pensando" e "Tudo em dia" sem "Iara pensando" |
| 4. Feed mostra "Para Davi" acima da mensagem; nenhum autor novo com Iara; resposta do Davi no feed | PASSOU | legenda na bolha; autores do feed |
| 4. Chip persiste no segundo envio (4b) | PASSOU | IPC `to=davi` no segundo envio |
| 5. Sem chip, envio vai à Iara sem "Para" | PASSOU | IPC `to=iara`; "Iara pensando"; sem legenda |
| 6. Clicar no cartão do Davi escolhe o destinatário e foca o campo | PASSOU | chip, cartão com aria-pressed, foco no campo |
| 7. Backspace com texto não remove o chip; Backspace com campo vazio remove | PASSOU | "xy" → "x" → "" com chip; depois sem chip |
| 7. × remove o chip | PASSOU | check no cenário 1 e no cenário 5 |
| 1c. "@Davi texto" + Enter envia ao Davi e cria o chip | PASSOU | IPC `to=davi` com texto sem "@Davi" |
| 8. Rede desligada (simulada via CDP) | PASSOU (simulado) | campo some e aparece "Sem conexão"; ao voltar, chip Davi intacto |

## 3. Lacunas

- Rede real desligada: NÃO VERIFICADO. O teste usou a emulação de rede do CDP, que só afeta a webview. O `check_connection` em Rust não foi exercitado com a rede real.
- Mutação: NÃO VERIFICADO. O pedido proíbe alterar código de produção; a prova de utilidade não foi feita.
- Tab para escolher na lista: NÃO VERIFICADO (não está no roteiro do pedido).
- Escolher Iara pelo chip (decisão 5 do plano): NÃO VERIFICADO.
- Delegação Iara → Davi com chip ausente: NÃO VERIFICADO (a resposta da Iara no cenário 5 não foi inspecionada).
- Temas Claro e Escuro (plano, item h) e roteiro de não regressão do PLAN Arquitetura (item i): NÃO VERIFICADO (fora do pedido).
- Campo digitável offline: não existe por desenho (`v-if="team.online"` em ActionBar.vue), então não há teste de digitação offline.
