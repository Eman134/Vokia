export type SettingsActionId = 'rever-onboarding' | 'abrir-estatisticas' | 'verificar-atualizacoes'

type SettingsAction = { label: string; variant: 'quiet' | 'danger'; id?: SettingsActionId }

type SettingsControl =
  | { kind: 'theme' }
  | { kind: 'permission-grant' }
  | { kind: 'execution-mode' }
  | { kind: 'auto-pause-on-limit' }
  | { kind: 'usage-pause-threshold' }
  | { kind: 'continuation-enabled' }
  | { kind: 'continuation-max-count' }
  | { kind: 'learning-mode' }
  | { kind: 'learning-turn-cadence' }
  | { kind: 'notifications-enabled' }
  | { kind: 'app-version' }
  | { kind: 'switch'; on: boolean }
  | { kind: 'segmented'; options: readonly string[]; selected: string }
  | { kind: 'select'; options: readonly string[] }
  | { kind: 'input'; value: string; unit?: string }
  | { kind: 'value'; text: string; note: string }
  | { kind: 'code'; values: readonly string[]; tone?: 'ok' | 'danger' }
  | { kind: 'keys'; keys: readonly string[] }
  | { kind: 'actions'; actions: readonly SettingsAction[] }
  | { kind: 'erase-data' }

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
  | { kind: 'live-usage' }
  | { kind: 'moments'; items: readonly SettingsMoment[] }
  | { kind: 'note'; text: string }

interface SettingsSectionData {
  title: string
  description: string
  action?: string
  items: readonly SettingsItem[]
}

export const GENERAL_SECTION_TITLE = 'Geral'
export const AGENTS_SECTION_TITLE = 'Agentes e conexões'
export const FALLBACK_SECTION_TITLE = 'Fallback e roteamento'
export const SEARCH_SECTION_TITLE = 'Pesquisa'

export interface SettingsNavGroup {
  label: string
  sections: readonly string[]
}

export const SETTINGS_NAV_GROUPS: readonly SettingsNavGroup[] = [
  {
    label: 'Aplicativo',
    sections: [GENERAL_SECTION_TITLE, 'Barra de ação e avisos', 'Atualizações', 'Privacidade e dados'],
  },
  {
    label: 'IA e Provedores',
    sections: [AGENTS_SECTION_TITLE, FALLBACK_SECTION_TITLE, SEARCH_SECTION_TITLE],
  },
  {
    label: 'Fluxo e Segurança',
    sections: ['Execução', 'Permissões', 'Memória e Skills', 'Uso e limites'],
  },
]

export const SETTINGS_SECTIONS: readonly SettingsSectionData[] = [
  {
    title: GENERAL_SECTION_TITLE,
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
        control: {
          kind: 'actions',
          actions: [{ label: 'Rever o onboarding', variant: 'quiet', id: 'rever-onboarding' }],
        },
      },
    ],
  },
  {
    title: AGENTS_SECTION_TITLE,
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
        kind: 'row',
        label: 'Pausar automaticamente ao atingir limite',
        hint: 'pausa o time e agenda retomada automática; desligado, avisa na conversa',
        control: { kind: 'auto-pause-on-limit' },
      },
      {
        kind: 'row',
        label: 'Continuação automática de turnos',
        hint: 'retoma turnos encerrados com etapas pendentes no plano, anúncio sem execução ou queda da CLI',
        control: { kind: 'continuation-enabled' },
      },
      {
        kind: 'row',
        label: 'Limite de continuações por pedido',
        hint: 'teto de retomadas automáticas antes de devolver o controle a você (padrão 5)',
        control: { kind: 'continuation-max-count' },
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
        hint: 'as micro-tarefas só vão para Pronto depois do seu ok',
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
      { kind: 'group', label: 'Modo de execução' },
      {
        kind: 'row',
        label: 'Como os pedidos de permissão são decididos',
        hint: 'vale para o projeto selecionado e volta a manual ao fechar o app; o mesmo seletor fica na barra de ações',
        control: { kind: 'execution-mode' },
      },
      { kind: 'group', label: 'Comandos' },
      {
        kind: 'row',
        label: 'Aprovação de comando',
        hint: 'vale pela sessão do agente ou só para aquela vez',
        control: { kind: 'permission-grant' },
      },
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
        label: 'Notificações nativas do sistema',
        hint: 'avisa quando uma decisão precisa de você com a janela fora de foco',
        control: { kind: 'notifications-enabled' },
      },
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
    title: 'Memória e Skills',
    description: 'revisão assíncrona de memórias, sugestões de skills e grafo do projeto',
    items: [
      {
        kind: 'row',
        label: 'Revisão automática de memórias e skills',
        hint: 'extrai aprendizados duráveis em segundo plano usando modelo leve',
        control: { kind: 'learning-mode' },
      },
      {
        kind: 'row',
        label: 'Cadência por turnos concluídos',
        hint: 'dispara revisão em background a cada N turnos (além de card concluído, ociosidade e limpeza)',
        control: { kind: 'learning-turn-cadence' },
      },
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
      { kind: 'live-usage' },
      {
        kind: 'row',
        label: 'Pausar o time quando a cota passar de',
        hint: 'retoma sozinho quando a janela renova',
        control: { kind: 'usage-pause-threshold' },
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
        control: { kind: 'actions', actions: [{ label: 'Abrir Estatísticas', variant: 'quiet', id: 'abrir-estatisticas' }] },
      },
    ],
  },
  {
    title: 'Atualizações',
    description: 'a Vokia se atualiza pelo GitHub Releases',
    items: [
      {
        kind: 'row',
        label: 'Versão instalada',
        hint: 'verificação automática ao abrir em builds de produção',
        control: { kind: 'app-version' },
      },
      {
        kind: 'row',
        label: 'Procurar atualização',
        hint: 'consulta o GitHub Releases agora',
        control: { kind: 'actions', actions: [{ label: 'Verificar agora', variant: 'quiet', id: 'verificar-atualizacoes' }] },
      },
    ],
  },
  {
    title: SEARCH_SECTION_TITLE,
    description: 'pesquisa web unificada para provedores sem busca nativa',
    items: [],
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
        hint: 'projetos, conversas, backlog e planos; inclui as configurações (tema e permissões voltam ao padrão); não mexe nos repositórios',
        control: { kind: 'erase-data' },
      },
    ],
  },
]
