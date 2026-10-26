# Resultado da validação: PLAN Arquitetura do projeto

Data: 2026-10-26
Escopo: Etapas 1 a 5 da arquitetura (seção "Validação de não regressão" do plano).
Fonte: notas de teste "Testes - Arquitetura Etapa 1, 2 e 4" (lidas com `maestri note read`) e registros do chat para a Etapa 3 (ver nota abaixo). Nada além disso foi usado.

## 1. Resultado por etapa

| Etapa | Estado | Comandos | Tauri dev | NÃO VERIFICADO |
|---|---|---|---|---|
| 1. Janela e bandeja (Rust) | PASSOU | fmt OK, clippy OK, typecheck OK, build OK | abre; bandeja; X; restaurar; segunda abertura; Sair | pedido à Iara, delegação, tema, CLI sem PATH (ver §2); clique físico; segunda abertura pelo Explorer |
| 2. Estado e comandos (Rust) | PASSOU | fmt OK, clippy OK, typecheck OK, build OK | abre; pedido à Iara responde; erro de rede no feed; CLI sem PATH mostra a tela | delegação ao Davi; tema nos três modos; rede desligada real (simulada por proxy) |
| 3. Pastas por feature (frontend) | NOTA NÃO ENCONTRADA | typecheck OK, build OK (relatado no chat) | abre; trilho, lista e feed; seletor muda de tema; console só com 404 de favicon | comparação visual; pedido à Iara falhou com error_max_turns (ver §3) |
| 4. Camada de serviço do Tauri | PASSOU | typecheck OK, build OK; `@tauri-apps` só nos serviços | CLI sem PATH; erro de rede no feed; checagem de atualização não roda em dev | comparação visual; tema nos três modos |
| 5. Documentar a estrutura | NÃO EXECUTADA | — | — | sem nota de etapa nem registro de validação |

fmt e clippy não foram rodados nas Etapas 3 e 4: não houve mudança em Rust nesses passos.

## 2. Roteiro do plano, item por item

| Item | Resultado | Etapa em que foi checado |
|---|---|---|
| App abre | PASSOU | 1, 2, 3, 4 |
| Pedido simples à Iara | PASSOU na Etapa 2 (respondeu). Na Etapa 3 e na Etapa 4 falhou com error_max_turns (ver §3) | 2 |
| Delegação ao Davi | NÃO VERIFICADO nas notas de arquitetura (a delegação foi observada depois, no teste de "Conversas entre agentes", que é outra validação) | — |
| Troca de tema nos três modos | PASSOU na Etapa 3 (relatado no chat: Claro, Escuro e Sistema). NÃO VERIFICADO nas Etapas 1, 2 e 4 | 3 |
| X esconde na bandeja | PASSOU | 1 |
| Clique no ícone restaura | PASSOU (via UI Automation Invoke, não clique físico) | 1 |
| Segunda abertura não cria instância | PASSOU (com janela oculta, minimizada e atrás de outra janela; disparada por PowerShell, não pelo Explorer) | 1 |
| Sair encerra | PASSOU | 1 |
| Sem CLI no PATH, a tela aparece | PASSOU | 2 e 4 |

## 3. Ocorrências

- **error_max_turns da Iara** (Etapa 3 e Etapa 4): a CLI saiu com código 1 e "Reached maximum number of turns (1)"; o erro chegou ao feed. Causa: `claude.rs` com `MAX_TURNS = "1"` e `--tools ""`. Essa ocorrência foi tratada fora da arquitetura.
- **Decisões aplicadas depois, sem nova validação de roteiro**: no working tree (`git diff`), `claude.rs` agora tem `MAX_TURNS = "3"` e `CLI_TIMEOUT = 120 s` com `kill_on_drop`. Essas mudanças não estão commitadas e não foram cobertas por este roteiro.
- **Espera sem limite (medida antes do timeout)**: com a rede bloqueada, a barra ficou em "Iara pensando" por cerca de 3 minutos antes do erro chegar ao feed (Etapa 4). Essa medição é anterior ao timeout de 120 s.

## 4. Nota sobre a Etapa 3

A nota "Testes - Arquitetura Etapa 3 - Frontend- pastas por feature" não existe mais na lista de notas (`maestri list`), e `maestri note read` retorna "not found". Os resultados da Etapa 3 acima vêm do relatório que enviei no chat e não foram conferidos contra nota. Refazer a nota é necessário para fechar esta etapa.

## 5. Pendências

- Restaurar ou refazer a nota da Etapa 3.
- Validar o roteiro de novo depois das mudanças de `MAX_TURNS` e timeout (não commitadas).
- Executar e registrar a Etapa 5 (documentação da estrutura).
- Delegação ao Davi e tema nos três modos, nas Etapas 1, 2 e 4.
