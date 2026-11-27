CREATE TABLE IF NOT EXISTS projects (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        path TEXT NOT NULL UNIQUE,
        selected INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER))
    , symbol TEXT NOT NULL DEFAULT '', description TEXT NOT NULL DEFAULT '', origin_kind TEXT NOT NULL DEFAULT 'folder', origin_repo TEXT, pinned INTEGER NOT NULL DEFAULT 0, permission_level TEXT NOT NULL DEFAULT 'liberado' CHECK (permission_level IN ('liberado','edicao','leitura')), color TEXT);

CREATE UNIQUE INDEX IF NOT EXISTS projects_one_selected ON projects(selected) WHERE selected = 1;

CREATE TABLE IF NOT EXISTS messages (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        project_id INTEGER NOT NULL REFERENCES projects(id),
        from_agent TEXT NOT NULL,
        to_agent TEXT NOT NULL,
        text TEXT NOT NULL,
        created_at INTEGER NOT NULL
    , channel_id INTEGER, activity TEXT, request_proposal TEXT, usage TEXT, task_result TEXT);

CREATE INDEX IF NOT EXISTS messages_by_project ON messages(project_id, id);

CREATE TABLE IF NOT EXISTS settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

CREATE TABLE IF NOT EXISTS agent_pauses (
        project_id INTEGER NOT NULL REFERENCES projects(id),
        agent TEXT NOT NULL,
        paused_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
        PRIMARY KEY (project_id, agent)
    );

CREATE TABLE IF NOT EXISTS permission_approvals (
        project_id INTEGER NOT NULL REFERENCES projects(id),
        tool TEXT NOT NULL,
        command TEXT NOT NULL,
        approved_count INTEGER NOT NULL DEFAULT 0,
        last_approved_at INTEGER NOT NULL,
        PRIMARY KEY (project_id, tool, command)
    );

CREATE TABLE IF NOT EXISTS agent_sessions (
        project_id INTEGER NOT NULL REFERENCES projects(id),
        agent TEXT NOT NULL,
        session_id TEXT NOT NULL,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (project_id, agent)
    );

CREATE TABLE IF NOT EXISTS backlog_items (
        project_id INTEGER NOT NULL REFERENCES projects(id),
        id TEXT NOT NULL,
        title TEXT NOT NULL,
        "type" TEXT NOT NULL,
        state TEXT NOT NULL,
        stage TEXT,
        triage_step INTEGER,
        parent TEXT,
        feature TEXT,
        description TEXT,
        plan TEXT,
        who TEXT NOT NULL,
        "by" TEXT NOT NULL,
        is_new INTEGER,
        "column" TEXT NOT NULL DEFAULT 'Backlog',
        priority TEXT,
        estimate TEXT,
        tags TEXT NOT NULL DEFAULT '[]',
        tasks TEXT NOT NULL DEFAULT '[]',
        criteria TEXT NOT NULL DEFAULT '[]',
        questions TEXT NOT NULL DEFAULT '[]',
        links TEXT NOT NULL DEFAULT '[]',
        activity TEXT NOT NULL DEFAULT '[]',
        children_total INTEGER,
        children_done INTEGER,
        position INTEGER NOT NULL, origin TEXT NOT NULL DEFAULT 'usuario',
        sprint_id TEXT,
        PRIMARY KEY (project_id, id)
    );

CREATE INDEX IF NOT EXISTS backlog_items_by_position ON backlog_items(project_id, position);
CREATE INDEX IF NOT EXISTS backlog_items_by_sprint ON backlog_items(project_id, sprint_id);

CREATE TABLE IF NOT EXISTS sprints (
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        id TEXT NOT NULL,
        name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
        goal TEXT NOT NULL DEFAULT '' CHECK (length(goal) <= 1000),
        starts_at INTEGER,
        ends_at INTEGER,
        status TEXT NOT NULL DEFAULT 'planejada' CHECK (status IN ('planejada','ativa','encerrada')),
        created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
        updated_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
        PRIMARY KEY (project_id, id)
    );

CREATE UNIQUE INDEX IF NOT EXISTS sprints_one_active ON sprints(project_id) WHERE status = 'ativa';

CREATE TABLE IF NOT EXISTS item_plans (
        project_id INTEGER NOT NULL,
        item_id TEXT NOT NULL,
        state TEXT NOT NULL,
        text TEXT,
        plan_id TEXT,
        plan_name TEXT,
        plan_meta TEXT,
        agent TEXT,
        linked_at INTEGER NOT NULL,
        PRIMARY KEY (project_id, item_id),
        FOREIGN KEY (project_id, item_id) REFERENCES backlog_items(project_id, id) ON DELETE CASCADE
    );

CREATE UNIQUE INDEX IF NOT EXISTS projects_one_pinned ON projects(pinned) WHERE pinned = 1;

CREATE TABLE IF NOT EXISTS channels (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        project_id INTEGER NOT NULL REFERENCES projects(id),
        name TEXT NOT NULL,
        members TEXT NOT NULL,
        created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
        UNIQUE (project_id, name)
    );

CREATE TABLE IF NOT EXISTS orch_settings (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        enforcement TEXT NOT NULL CHECK (enforcement IN ('off','on')),
        leader_agent TEXT NOT NULL,
        step_mode TEXT NOT NULL CHECK (step_mode IN ('lotes','livre')),
        human_gate TEXT NOT NULL CHECK (human_gate IN ('lote','fim','nunca')),
        max_parallel INTEGER NOT NULL CHECK (max_parallel BETWEEN 1 AND 8),
        pair_turn_limit INTEGER NOT NULL CHECK (pair_turn_limit BETWEEN 1 AND 50),
        task_turn_limit INTEGER NOT NULL CHECK (task_turn_limit BETWEEN 5 AND 200),
        worktree_policy TEXT NOT NULL CHECK (worktree_policy IN ('sempre','paralelo','nunca'))
    );

CREATE TABLE IF NOT EXISTS orch_layers (
        id TEXT PRIMARY KEY CHECK (id <> 'voce'),
        name TEXT NOT NULL,
        description TEXT NOT NULL,
        position INTEGER NOT NULL,
        kind TEXT NOT NULL CHECK (kind IN ('planejar','implementar','revisar','pesquisar','livre')),
        instructions TEXT NOT NULL CHECK (length(instructions) <= 4000)
    );

CREATE TABLE IF NOT EXISTS orch_teams (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        position INTEGER NOT NULL,
        leader_agent TEXT,
        instructions TEXT NOT NULL DEFAULT '' CHECK (length(instructions) <= 4000)
    );

CREATE TABLE IF NOT EXISTS orch_placements (
        agent_id TEXT PRIMARY KEY,
        team_id TEXT NOT NULL REFERENCES orch_teams(id),
        layer_id TEXT NOT NULL REFERENCES orch_layers(id),
        instructions TEXT NOT NULL CHECK (length(instructions) <= 4000),
        max_instances INTEGER NOT NULL CHECK (max_instances BETWEEN 1 AND 4)
    );

CREATE TABLE IF NOT EXISTS orch_handoffs (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        run_id INTEGER,
        from_agent TEXT NOT NULL,
        to_agent TEXT NOT NULL,
        from_layer TEXT,
        to_layer TEXT,
        action TEXT NOT NULL CHECK (action IN ('delegar','devolver','escalar','consultar')),
        kind TEXT NOT NULL CHECK (kind IN ('planejar','implementar','testar','revisar','pesquisar')),
        item_id TEXT,
        return_count INTEGER NOT NULL CHECK (return_count >= 0),
        message_id INTEGER,
        created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
        event TEXT,
        edge_id INTEGER
    );

CREATE INDEX IF NOT EXISTS orch_handoffs_by_project ON orch_handoffs(project_id, id);
