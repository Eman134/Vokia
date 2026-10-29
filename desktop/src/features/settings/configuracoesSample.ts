type SettingsAction = { label: string; variant: 'quiet' | 'danger' }

type SettingsControl =
  | { kind: 'theme' }
  | { kind: 'switch'; on: boolean }
  | { kind: 'segmented'; options: readonly string[]; selected: string }
  | { kind: 'select'; options: readonly string[] }
  | { kind: 'input'; value: string; unit?: string }
  | { kind: 'value'; text: string; note: string }
  | { kind: 'code'; values: readonly string[]; tone?: 'ok' | 'danger' }
  | { kind: 'keys'; keys: readonly string[] }
  | { kind: 'actions'; actions: readonly SettingsAction[] }

interface SettingsMoment {
  label: string
  hint: string
  on: boolean
}

export type SettingsItem =
  | { kind: 'group'; label: string; hint?: string }
  | { kind: 'row'; label: string; hint?: string; control: SettingsControl }
  | { kind: 'cli'; code: string; name: string; version: string; status: string; detail: string; action: string }
  | { kind: 'usage'; label: string; detail: string; percent: number }
  | { kind: 'moments'; items: readonly SettingsMoment[] }
  | { kind: 'note'; text: string }

interface SettingsSectionData {
  title: string
  description: string
  action?: string
  items: readonly SettingsItem[]
}

export const SETTINGS_SECTIONS: readonly SettingsSectionData[] = [
  {
    title: 'Geral',
    description: 'aparência e o básico do app',
    items: [
      { kind: 'row', label: 'Tema', control: { kind: 'theme' } },
      {
        kind: 'row',
        label: 'Idioma',
        control: { kind: 'select', options: ['Português (Brasil)', 'English', 'Español'] },
      },
      {
        kind: 'row',
        label: 'Densidade',
        hint: 'compacta mostra mais cards no kanban e no mapa',
        control: { kind: 'segmented', options: ['Confortável', 'Compacta'], selected: 'Confortável' },
      },
      {
        kind: 'row',
        label: 'Abrir a Vokia ao iniciar o computador',
        hint: 'o time retoma de onde parou',
        control: { kind: 'switch', on: false },
      },
      {
        kind: 'row',
        label: 'Atalhos',
        hint: 'procurar, mencionar um agente, pausar o time',
        control: { kind: 'keys', keys: ['⌘K', '@', 'Espaço'] },
      },
      {
        kind: 'row',
        label: 'Tour e configuração inicial',
        hint: 'as seis etapas de boas-vindas, conexões, projeto, time, pedido e pronto',
        control: { kind: 'actions', actions: [{ label: 'Rever o onboarding', variant: 'quiet' }] },
      },
    ],
  },
  {
    title: 'Agentes e conexões',
    description: 'os CLIs que a Vokia usa; o login é sempre o do próprio CLI',
    action: 'Verificar de novo',
    items: [
      {
        kind: 'cli',
        code: 'CC',
        name: 'Claude Code',
        version: 'v2.1.4',
        status: 'Logado',
        detail: '· plano Max · Opus e Sonnet',
        action: 'Testar',
      },
      {
        kind: 'cli',
        code: 'CX',
        name: 'Codex',
        version: 'v0.48.0',
        status: 'Logado',
        detail: '· conta ChatGPT Plus',
        action: 'Testar',
      },
      {
        kind: 'cli',
        code: 'GH',
        name: 'GitHub CLI',
        version: 'v2.62',
        status: 'Logado como @kayke',
        detail: '· clonar, PRs e issues',
        action: 'Trocar conta',
      },
      {
        kind: 'cli',
        code: 'G',
        name: 'Git',
        version: '2.47',
        status: 'Pronto',
        detail: '· user.name Kayke',
        action: 'Ver config',
      },
      { kind: 'group', label: 'Modelo padrão por camada' },
      {
        kind: 'row',
        label: 'Liderança',
        hint: 'planeja, divide em cards e revisa lotes',
        control: { kind: 'select', options: ['Claude Code · Opus', 'Claude Code · Sonnet', 'Codex · GPT-5'] },
      },
      {
        kind: 'row',
        label: 'Interface e Serviços',
        hint: 'implementação; cada agente pode trocar em Agentes',
        control: { kind: 'select', options: ['Claude Code · Sonnet', 'Codex · GPT-5', 'Claude Code · Opus'] },
      },
      {
        kind: 'row',
        label: 'Qualidade',
        hint: 'revisão e testes',
        control: { kind: 'select', options: ['Claude Code · Sonnet', 'Codex · GPT-5'] },
      },
      {
        kind: 'row',
        label: 'Pesquisa',
        hint: 'comparar, pesquisar e resumir',
        control: { kind: 'select', options: ['Codex · GPT-5', 'Claude Code · Sonnet'] },
      },
      {
        kind: 'row',
        label: 'Micro-tarefas',
        hint: 'o que a líder desdobra de uma feature; ela sobe o modelo se a tarefa pedir',
        control: { kind: 'select', options: ['Claude Code · Haiku', 'Codex · GPT-5 mini', 'Claude Code · Sonnet'] },
      },
      {
        kind: 'note',
        text: 'A Vokia nunca lê nem guarda tokens. Para trocar de conta, use o próprio CLI e clique em Verificar de novo.',
      },
    ],
  },
  {
    title: 'Execução',
    description: 'quanto o time roda ao mesmo tempo e quando para',
    items: [
      {
        kind: 'row',
        label: 'Agentes ao mesmo tempo',
        hint: 'somando todos os projetos',
        control: { kind: 'input', value: '4' },
      },
      {
        kind: 'row',
        label: 'Devoluções antes de chamar você',
        hint: '1ª: a líder detalha o roteiro e o mesmo modelo tenta de novo · 2ª: sobe o modelo · depois disso, decisão para você. O que faltou vira fato na memória. Igual em todas as setas da Orquestração.',
        control: { kind: 'input', value: '2', unit: 'vezes' },
      },
      {
        kind: 'row',
        label: 'Tempo máximo por tarefa',
        hint: 'o agente pausa e explica onde parou',
        control: { kind: 'input', value: '30', unit: 'min' },
      },
      {
        kind: 'group',
        label: 'Precisa do meu ok',
        hint: 'um lugar só para o que espera você; o resto anda sozinho',
      },
      {
        kind: 'row',
        label: 'Classificação do pedido',
        hint: 'a barra mostra "parece pequeno / parece feature" e espera o clique',
        control: { kind: 'switch', on: true },
      },
      {
        kind: 'row',
        label: 'Proposta da feature',
        hint: 'as micro-tarefas só vão para Ready depois do seu ok',
        control: { kind: 'switch', on: true },
      },
      {
        kind: 'row',
        label: 'A cada lote',
        hint: 'o próximo lote só começa com o seu ok',
        control: { kind: 'switch', on: true },
      },
      {
        kind: 'row',
        label: 'Merge e push',
        hint: 'PR e merge sempre passam por você',
        control: { kind: 'switch', on: true },
      },
      {
        kind: 'row',
        label: 'Exigir testes antes de concluir um card',
        hint: 'Qualidade roda a suíte e anexa a evidência',
        control: { kind: 'switch', on: true },
      },
      { kind: 'group', label: 'Worktrees' },
      {
        kind: 'row',
        label: 'Pasta dos worktrees',
        hint: 'um por feature, fora da sua branch',
        control: { kind: 'code', values: ['.vokia/worktrees/'] },
      },
      {
        kind: 'row',
        label: 'Prefixo das branches',
        hint: 'ex.: vokia/pagamento-pix',
        control: { kind: 'code', values: ['vokia/'] },
      },
      {
        kind: 'row',
        label: 'Apagar worktrees de features concluídas',
        hint: 'depois do merge, mantém a branch',
        control: { kind: 'switch', on: true },
      },
    ],
  },
  {
    title: 'Permissões',
    description: 'o que os agentes fazem sozinhos e o que pede a sua aprovação',
    items: [
      { kind: 'group', label: 'Pedir aprovação antes de' },
      {
        kind: 'row',
        label: 'Instalar dependências',
        hint: 'npm, pnpm, pip e afins',
        control: { kind: 'switch', on: true },
      },
      { kind: 'row', label: 'git push e abrir PR', control: { kind: 'switch', on: true } },
      {
        kind: 'row',
        label: 'Acessar a internet',
        hint: 'fora da documentação liberada',
        control: { kind: 'switch', on: true },
      },
      { kind: 'row', label: 'Mexer fora do worktree', control: { kind: 'switch', on: true } },
      { kind: 'row', label: 'Apagar arquivos', control: { kind: 'switch', on: true } },
      { kind: 'group', label: 'Comandos' },
      {
        kind: 'row',
        label: 'Sempre permitidos',
        control: { kind: 'code', values: ['npm test', 'pnpm lint', 'git status'], tone: 'ok' },
      },
      {
        kind: 'row',
        label: 'Sempre bloqueados',
        control: { kind: 'code', values: ['rm -rf /', 'git push --force'], tone: 'danger' },
      },
      {
        kind: 'note',
        text: 'Cada agente pode ter regras próprias em Agentes › Permissões. As aprovações chegam na barra de ação como decisões.',
      },
    ],
  },
  {
    title: 'Barra de ação e avisos',
    description: 'como a barra de baixo, a chama e as notificações se comportam',
    items: [
      {
        kind: 'row',
        label: 'Modo da barra',
        hint: 'automática abre ao passar o mouse ou focar no campo',
        control: { kind: 'segmented', options: ['Automática', 'Compacta', 'Sempre aberta'], selected: 'Automática' },
      },
      {
        kind: 'row',
        label: 'Fora do app',
        hint: 'com a Vokia minimizada, a barra fica no Windows: na bandeja, na barra de tarefas ou presa na lateral',
        control: {
          kind: 'segmented',
          options: ['Desligada', 'Ícone na bandeja', 'Na barra de tarefas', 'Lateral da tela'],
          selected: 'Ícone na bandeja',
        },
      },
      {
        kind: 'row',
        label: 'Mostrar a chama na barra',
        hint: 'muda com o estado: trabalhando, pensando, precisa de você, concluído, erro',
        control: { kind: 'switch', on: true },
      },
      {
        kind: 'row',
        label: 'Mostrar fluxos em etapas na barra',
        hint: 'onboarding e novo projeto com anel de progresso',
        control: { kind: 'switch', on: true },
      },
      { kind: 'group', label: 'Momentos' },
      {
        kind: 'moments',
        items: [
          { label: 'Memória', hint: 'fato novo numa conversa', on: true },
          { label: 'Grafo', hint: 'arquivos ou artefatos ligados', on: true },
          { label: 'Decisão', hint: 'algo que vira ADR', on: true },
          { label: 'Skill', hint: 'um jeito de trabalhar repetido', on: true },
          { label: 'Conflito', hint: 'dois agentes no mesmo arquivo', on: true },
          { label: 'Festa', hint: 'feature concluída', on: false },
        ],
      },
      {
        kind: 'row',
        label: 'Quanto tempo o balão fica',
        hint: 'sem resposta vai para Memória › Sugestões',
        control: { kind: 'input', value: '8', unit: 's' },
      },
      { kind: 'group', label: 'Notificações do sistema' },
      {
        kind: 'row',
        label: 'Decisão esperando você',
        control: { kind: 'switch', on: true },
      },
      {
        kind: 'row',
        label: 'Erro ou teste falhando',
        control: { kind: 'switch', on: true },
      },
      {
        kind: 'row',
        label: 'Lote ou feature concluída',
        control: { kind: 'switch', on: false },
      },
      {
        kind: 'row',
        label: 'Não perturbe',
        hint: 'a chama continua mudando, só os avisos esperam',
        control: { kind: 'segmented', options: ['Desligado', '22h às 8h', 'Em foco'], selected: '22h às 8h' },
      },
    ],
  },
  {
    title: 'Memória e grafo',
    description: 'o que o time aprende e o mapa que liga código e artefatos',
    items: [
      {
        kind: 'row',
        label: 'Confirmar antes de guardar um fato',
        hint: 'sugestões dos agentes esperam o seu ok',
        control: { kind: 'switch', on: true },
      },
      {
        kind: 'row',
        label: 'Extrair fatos de artefatos',
        hint: 'diagramas, documentos e ADRs viram fatos com origem',
        control: { kind: 'switch', on: true },
      },
      {
        kind: 'row',
        label: 'Aprender suas preferências',
        hint: 'ex.: "nunca instalar dependência sem pedir"',
        control: { kind: 'switch', on: true },
      },
      {
        kind: 'row',
        label: 'Atualizar o grafo a cada commit',
        control: { kind: 'switch', on: true },
      },
      {
        kind: 'row',
        label: 'Avisar artefato desatualizado',
        hint: 'quando um arquivo ligado muda e o artefato não acompanha',
        control: { kind: 'switch', on: true },
      },
      {
        kind: 'row',
        label: 'Onde fica',
        hint: 'Markdown e JSON versionados com o código',
        control: { kind: 'code', values: ['.vokia/memory/', '.vokia/artifacts/links.json'] },
      },
      {
        kind: 'row',
        label: 'Reindexar o grafo',
        hint: 'loja-online · 1.284 arquivos · há 3 min',
        control: { kind: 'actions', actions: [{ label: 'Reindexar agora', variant: 'quiet' }] },
      },
    ],
  },
  {
    title: 'Uso e limites',
    description: 'o custo é dos seus planos; a Vokia mede e segura o time antes do limite',
    items: [
      { kind: 'usage', label: 'Claude Code · janela de 5 h', detail: '62% · renova em 1 h 40', percent: 62 },
      { kind: 'usage', label: 'Claude Code · semana', detail: '41%', percent: 41 },
      { kind: 'usage', label: 'Codex · janela de 5 h', detail: '28% · renova em 3 h 10', percent: 28 },
      {
        kind: 'row',
        label: 'Pausar o time quando passar de',
        hint: 'retoma sozinho quando a janela renova',
        control: { kind: 'input', value: '90', unit: '%' },
      },
      {
        kind: 'row',
        label: 'Avisar quando passar de',
        control: { kind: 'input', value: '75', unit: '%' },
      },
      {
        kind: 'row',
        label: 'Modo econômico perto do limite',
        hint: 'liderança passa para Sonnet e o grafo corta o contexto',
        control: { kind: 'switch', on: true },
      },
      {
        kind: 'row',
        label: 'Relatório completo',
        hint: 'gasto por agente, projeto e camada',
        control: { kind: 'actions', actions: [{ label: 'Abrir Estatísticas', variant: 'quiet' }] },
      },
    ],
  },
  {
    title: 'Atualizações',
    description: 'a Vokia se atualiza pelo GitHub Releases',
    action: 'Verificar agora',
    items: [
      {
        kind: 'row',
        label: 'Versão',
        hint: 'verificado hoje às 14:02',
        control: { kind: 'value', text: '0.9.2', note: 'a mais recente' },
      },
      {
        kind: 'row',
        label: 'Canal',
        control: { kind: 'segmented', options: ['Estável', 'Beta'], selected: 'Estável' },
      },
      {
        kind: 'row',
        label: 'Buscar atualizações ao abrir',
        control: { kind: 'switch', on: true },
      },
      {
        kind: 'row',
        label: 'Instalar ao fechar o app',
        hint: 'nunca reinicia com o time trabalhando',
        control: { kind: 'switch', on: true },
      },
      {
        kind: 'row',
        label: 'Notas da versão',
        hint: 'o que mudou na 0.9.2',
        control: { kind: 'actions', actions: [{ label: 'Ver notas', variant: 'quiet' }] },
      },
    ],
  },
  {
    title: 'Privacidade e dados',
    description: 'tudo fica no seu computador',
    items: [
      {
        kind: 'row',
        label: 'Telemetria de uso',
        hint: 'contagens anônimas, sem código, prompts ou conversas',
        control: { kind: 'switch', on: false },
      },
      {
        kind: 'row',
        label: 'Relatórios de erro',
        hint: 'só a pilha do erro e a versão',
        control: { kind: 'switch', on: false },
      },
      {
        kind: 'row',
        label: 'Dados do app',
        hint: 'configurações, times e histórico das conversas',
        control: { kind: 'code', values: ['~/.vokia'] },
      },
      {
        kind: 'row',
        label: 'Configurações',
        hint: 'leve times, permissões e preferências para outro computador',
        control: {
          kind: 'actions',
          actions: [
            { label: 'Exportar', variant: 'quiet' },
            { label: 'Importar', variant: 'quiet' },
          ],
        },
      },
      {
        kind: 'row',
        label: 'Apagar dados do app',
        hint: 'não mexe nos repositórios nem na pasta .vokia/ deles',
        control: { kind: 'actions', actions: [{ label: 'Apagar…', variant: 'danger' }] },
      },
    ],
  },
]
