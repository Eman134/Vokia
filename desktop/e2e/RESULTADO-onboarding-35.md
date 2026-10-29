# RESULTADO · Etapa 35 · Onboarding (6 telas)

Veredito: **PASSOU** — 168 de 168 verificações (Edge headless via CDP, Vite de teste com mocks do Tauri).

## Como repetir

```
cd desktop
bash e2e/onboarding-rodar.sh
```

Sobe o próprio Vite na porta 4196 e o Edge com CDP na 9356, roda `e2e/onboarding.cdp.mjs` e para só esses dois processos. Saída em `e2e/evidencias-onboarding/` (`run.log`, `resultados.json`, capturas PNG). Código de saída 0 quando tudo passa.

## Formas de falha consideradas

- Trilho aparecendo ou deixando coluna vazia no onboarding; trilho não voltando depois.
- Tema mudando e a tela ficando ilegível em Escuro (e em Claro).
- Passo travado ao sair e voltar pelo trilho.
- Barra de ação cobrindo o botão final nas telas altas.
- Voltar existindo na primeira tela; Continuar/Voltar/Pular indo para a tela errada.
- Controle de exemplo reagindo ao clique (Enviar para a Iara, cartões, origem, sugestões).
- Coluna fora de 600 px ou descentralizada; rolagem horizontal; marcador "dados de exemplo" ausente.

## Cobertura (por cenário)

| Cenário | Verificações | O que confere |
|---|---|---|
| Telas 1440x900 e 1200x780 | 61 + 61 | Em cada uma das 6 telas: título e subtítulo exatos, marca, título 34 px / linha 1.1, coluna de 600 px centralizada com a casca em 100% da largura (`--rail-w` 0 px), marcador, "Pular", rodapé (Voltar/botão final e estado), controles de exemplo desabilitados, botão final fora da barra de ação com a página rolada ao fim, sem rolagem horizontal |
| Navegação | 18 | Continuar 1→5, Voltar 5→1, Voltar ausente na 1ª, "Pular" a partir de cada uma das 5 telas vai a Pronto, "Começar sem time" vai a Pedido, Voltar em Pronto vai a Pedido, "Enviar para a Iara" desabilitado sem efeito, dicas de Pronto |
| Trilho | 6 | "Abrir o mapa do time" → Mapa do time com trilho de volta (88 px); "Novo projeto" reabre na 1ª tela sem trilho; passo não trava; segundo ciclo (Continuar até Pedido, Pular, Abrir o mapa) devolve o trilho |
| Tema | 21 | ThemeSwitch real: data-theme e opção marcada para Escuro/Claro/Sistema, fundo muda, contraste (título, subtítulo, botão final, marcador) ≥ 4.5 nas 6 telas em Escuro e em Claro (mínimo medido 5.04), tema mantido ao ir a Pronto e voltar |
| Console | 1 | Nenhuma exceção nem `console.error` |

Critérios de aceite: 1 (telas, casca, sem trilho), 2 (navegação), 3 (tema), 4 (mapa com trilho) e 5 (controles desabilitados e marcador) cobertos.

## Observações (não são falha)

- Pedido: o botão "Enviar para a Iara" é desabilitado por contrato ("Não fazer: enviar pedido"). Nessa tela só avançam "Pular e ir para o fim" e Voltar; o E2E usa o "Pular" para chegar a Pronto.
- Boas-vindas não mostra "dados de exemplo" (não tem dado de exemplo); as outras cinco mostram. O E2E trava esse comportamento.
- Com 780 px de altura o contêiner rola e a barra de rolagem (14 px) desloca o centro visível em 7 px; o centro é medido por `clientWidth`.
- O onboarding sempre reinicia na 1ª tela ao ser reaberto (a página é desmontada).

## Lacunas

- Prova de utilidade por mutação não feita: o pedido proibiu alterar código de produção. As verificações comparam valores exatos (textos, medidas, estado de `disabled`, `--rail-w`, h1 da tela de destino), e uma regressão como trilho de volta no onboarding, `showRail` quebrado ou Voltar na 1ª tela as reprovaria.
- Detalhe visual lado a lado com `docs/prototipo/Onboarding.html` não foi comparado por este E2E (capturas em `evidencias-onboarding/`).
- Typecheck e build não foram rodados aqui (fora do pedido).
