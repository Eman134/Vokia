# Resultado da validação visual: Tokens Etapa 6 (travar namespaces)

Data: 26/10/2026
Método: `npm run tauri dev` (WebView2, DevTools via CDP) e buscas em `desktop/src`. Mensagens de teste inseridas no store em dev.
Temas: Claro, Escuro e Sistema, com o Windows em escuro e (só no Sistema) em claro. O Windows foi restaurado para escuro ao final.
Capturas: `scratchpad` da sessão do Tester (`etapa6-*.png`); não versionadas.

## 1. Buscas do critério (fora de `tokens.css`)

| Busca | Resultado |
|---|---|
| Cor literal (`#`, `rgb(`, `rgba(`, `hsl(`) | vazio |
| Valor arbitrário entre colchetes em classe Tailwind | vazio |
| Classes padrão removidas (`text-xs`, `text-sm`, `text-lg`, `rounded-xl`, `shadow-lg`, etc.) | vazio |
| Paleta padrão (`white`, `gray`, `red`, etc.) | vazio |

Dentro de `tokens.css`: `--color-*`, `--radius-*`, `--text-*` e `--shadow-*` estão com `initial`, e `transparent` e `current` foram recriados.
No CSS gerado (`dist`): `.rounded-full`, `.rounded-tr-tail`, `.text-small`, `.shadow-float`, `.shadow-seg` presentes; `.rounded-tr-sm`, `.rounded-xl`, `.shadow-lg`, `.text-sm`, `--color-gray-*` ausentes.

## 2. Build

- `npm run typecheck`: sem erro.
- `npm run build`: sem erro.

## 3. Valores computados (tema Escuro, Windows escuro)

| Componente | Propriedade | Esperado (tabela) | Encontrado |
|---|---|---|---|
| Body | fundo | 121214 | rgb(18, 18, 20) |
| Body | texto | ececf0 | rgb(236, 236, 240) |
| Body | fonte / entrelinha | 14px / 1.55 | 14px / 21.7px |
| Balão do usuário | raio TL / TR / BR / BL | 14 / 4 (tail) / 14 / 14 | 14 / 4 / 14 / 14 |
| Balão do usuário | fonte, largura máxima, padding | 13.5px, 75%, 16px | 13.5px, 75%, 16px |
| Pílula da barra | raio, altura, sombra | rounded-full, 48, float | 9999px, 48, float |
| Ponto de estado | tamanho, raio | 7 px, redondo | 7×7, redondo |
| Avatar | tamanho, raio | 28 px, redondo | 28 px, redondo |
| Seletor (item selecionado) | raio, peso, sombra | 8, 500, seg | 8px, 500, seg (0 1px 2px 8%) |
| Seletor (grupo) | fundo, raio | surface-2, 10 | rgb(35, 35, 40), 10px |
| Título (Conversas) | tamanho, peso, espaço, entrelinha | 44, 600, -0,035em, 1.05 | 44px, 600, -1.54px, 46.2px |
| Superfície (barra) | fundo | surface 1a1a1e | rgb(26, 26, 30) |

## 4. Pares de contraste (calculados a partir dos valores de token)

| Par | Claro | Escuro | Plano | Resultado |
|---|---|---|---|---|
| muted sobre surface-2 | 4.75 | 6.11 | ≥ 4,5 | atende |
| accent-ink sobre accent | 5.63 | 8.46 | ≥ 4,5 | atende |
| danger sobre surface | 4.47 | 7.04 | 4,47 registrado | registrado |

Observação: `--color-accent-ink` não aparece no CSS gerado porque nenhuma classe o usa (o Tailwind só emite variáveis referenciadas). O par foi calculado a partir dos hex do token, não medido no DOM.

## 5. Modos de tema (body e superfície)

| Modo | Windows | Body | Superfície (barra) | Resultado |
|---|---|---|---|---|
| Claro | escuro | rgb(250, 250, 250) | rgb(255, 255, 255) | igual ao token claro |
| Escuro | escuro | rgb(18, 18, 20) | rgb(26, 26, 30) | igual ao token escuro |
| Sistema | escuro | rgb(18, 18, 20) | rgb(26, 26, 30) | segue o Windows |
| Sistema | claro | rgb(250, 250, 250) | rgb(255, 255, 255) | segue o Windows |

## 6. Mudança de namespace no balão do usuário

O canto superior direito agora usa `rounded-tr-tail` (4 px), igual ao `rounded-tr-sm` anterior (4 px). Medido: 4px. Sem mudança visível.

## 7. NÃO VERIFICADO

- Comparação lado a lado com `docs/prototipo/Design.html`: não feita. O protótipo tem 6,2 MB e não foi aberto no Chrome desta sessão.
- Temas Claro e Escuro com o Windows em claro: só o Sistema foi medido nesse modo.
- Clique físico do mouse: não usado (seletor acionado por JS no DOM).
- Captura de tela anexada ao repositório: não feita (capturas ficaram fora do repositório).
