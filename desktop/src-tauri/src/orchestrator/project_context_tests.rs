use super::*;

#[test]
fn tok2_ember_recebe_identidade_sem_bloco_da_arvore() {
    let project = Project {
        id: 1,
        name: "Vokia".to_string(),
        selected: true,
        created_at: 0,
        symbol: "folder".to_string(),
        color: Some("amber".to_string()),
        description: "Orquestrador de agentes".to_string(),
        path: "C:/projetos/Vokia".to_string(),
        pinned: true,
        permission_level: crate::permissions::PermissionLevel::Edicao,
        origin_kind: ORIGIN_GITHUB.to_string(),
        origin_repo: Some("org/vokia".to_string()),
        last_activity_at: None,
        setup_command: None,
    };
    let ember = project_context_without_tree(&project);

    assert!(ember.contains("Vokia"));
    assert!(ember.contains("Orquestrador de agentes"));
    assert!(ember.contains("org/vokia"));
    assert!(!ember.contains("Estrutura da pasta do projeto"));
}

#[test]
fn tok2_arvore_respeita_profundidade_limite_de_entradas_e_soma_omissoes() {
    let mut entries: Vec<String> = (0..75).map(|index| format!("arquivo-{index:02}.txt")).collect();
    entries.push("desktop/src/App.vue".to_string());
    let block = project_tree_block(&FolderTree {
        entries,
        omitted: 3,
        omitted_is_lower_bound: false,
    });
    let lines: Vec<_> = block.lines().collect();

    assert!(lines[0].contains("até 2 níveis"));
    assert_eq!(lines.iter().filter(|line| line.starts_with("arquivo-")).count(), 60);
    assert!(!block.contains("desktop/src/App.vue"));
    assert!(block.contains("… e mais 19 entradas não listadas."));
}

#[test]
fn tok2_arvore_respeita_orcamento_total_de_1800_caracteres() {
    let entries: Vec<String> = (0..70)
        .map(|index| format!("diretorio-com-nome-longo-subdiretorio-{index:02}.txt"))
        .collect();
    let block = project_tree_block(&FolderTree {
        entries,
        omitted: 0,
        omitted_is_lower_bound: false,
    });

    assert!(block.chars().count() <= PROMPT_TREE_MAX_CHARS);
}

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vokia-tree-cache-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn tok4_scan_structure_reaproveita_arvore_dentro_do_ttl() {
    let dir = scratch_dir("hit");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), "a").unwrap();

    let first = tauri::async_runtime::block_on(scan_structure(&dir)).expect("primeira leitura");
    std::fs::write(dir.join("b.txt"), "b").unwrap();
    let second = tauri::async_runtime::block_on(scan_structure(&dir)).expect("leitura em cache");

    assert_eq!(first.entries, vec!["a.txt".to_string()]);
    assert_eq!(second.entries, first.entries);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tok4_scan_structure_rele_a_pasta_depois_do_ttl() {
    let dir = scratch_dir("expired");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    let first = tauri::async_runtime::block_on(scan_structure(&dir)).expect("primeira leitura");
    std::fs::write(dir.join("b.txt"), "b").unwrap();

    let expired = Instant::now()
        .checked_sub(TREE_CACHE_TTL + Duration::from_secs(1))
        .expect("relógio monotônico com mais de 31s");
    tree_cache().lock().unwrap().insert(dir.clone(), (expired, first));
    let refreshed = tauri::async_runtime::block_on(scan_structure(&dir)).expect("nova leitura");

    assert_eq!(refreshed.entries, vec!["a.txt".to_string(), "b.txt".to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tok4_scan_structure_nao_guarda_erro_no_cache() {
    let dir = scratch_dir("error");

    assert!(tauri::async_runtime::block_on(scan_structure(&dir)).is_none());
    assert!(cached_tree(&dir).is_none());

    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    let recovered = tauri::async_runtime::block_on(scan_structure(&dir)).expect("leitura após criar a pasta");
    assert_eq!(recovered.entries, vec!["a.txt".to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}

fn project_at(dir: &std::path::Path) -> Project {
    Project {
        id: 1,
        name: "Vokia".to_string(),
        selected: true,
        created_at: 0,
        symbol: "folder".to_string(),
        color: None,
        description: String::new(),
        path: dir.to_string_lossy().into_owned(),
        pinned: false,
        permission_level: crate::permissions::PermissionLevel::Edicao,
        origin_kind: "folder".to_string(),
        origin_repo: None,
        last_activity_at: None,
        setup_command: None,
    }
}

fn instructions_dir(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("vokia-root-instructions-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir
}

#[test]
fn instrucoes_da_raiz_entram_no_contexto_de_quem_trabalha_mas_nao_no_do_ember() {
    let dir = instructions_dir("ambos", &[("CLAUDE.md", "Regra do Claude"), ("AGENTS.md", "Regra dos agentes")]);
    let project = project_at(&dir);

    let worker = project_context(&project, None);
    let ember = project_context_without_tree(&project);

    assert!(worker.contains("CLAUDE.md") && worker.contains("Regra do Claude"));
    assert!(worker.contains("AGENTS.md") && worker.contains("Regra dos agentes"));
    assert!(!ember.contains("Regra do Claude"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn instrucoes_acima_do_teto_entram_cortadas_e_marcadas() {
    let long = "x".repeat(ROOT_INSTRUCTIONS_MAX_CHARS + 500);
    let dir = instructions_dir("longo", &[("CLAUDE.md", &long)]);

    let context = project_context(&project_at(&dir), None);

    assert!(context.contains(ROOT_INSTRUCTIONS_TRUNCATED));
    assert!(context.contains(&"x".repeat(ROOT_INSTRUCTIONS_MAX_CHARS)));
    assert!(!context.contains(&"x".repeat(ROOT_INSTRUCTIONS_MAX_CHARS + 1)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sem_arquivo_ou_com_arquivo_vazio_nao_ha_secao_de_instrucoes() {
    let empty = instructions_dir("vazio", &[("CLAUDE.md", "  \n")]);
    let none = instructions_dir("nenhum", &[]);

    for dir in [&empty, &none] {
        assert!(!project_context(&project_at(dir), None).contains(ROOT_INSTRUCTIONS_LABEL));
        let _ = std::fs::remove_dir_all(dir);
    }
}
