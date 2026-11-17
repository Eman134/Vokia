use serde_json::{json, Value};

use super::backlog_tests::{world, Caller, World};
use super::*;
use crate::task::{Change, VarValue};

const NO_VARIABLES: &str =
    "Esta tarefa não usa variáveis: não há o que registrar. Responda só com texto.";
const EMPTY_LISTING: &str = "- nenhuma";

fn open_exchange(who: &Caller) {
    who.slot.begin(None, None);
    who.slot.enable_variables();
}

fn set(world: &World, who: &Caller, name: &str, value: Value) -> super::backlog_tests::Outcome {
    world.call(
        who,
        "set_variable",
        json!({ "name": name, "op": "set", "value": value }),
    )
}

fn listing(world: &World, who: &Caller) -> String {
    world.call(who, "get_variables", json!({})).text
}

#[test]
fn sem_variaveis_na_troca_set_variable_explica_e_nada_e_gravado() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);

    iara.slot.begin(None, None);
    let outcome = set(&world, &iara, "contagem", json!(1));

    assert!(outcome.is_error);
    assert_eq!(outcome.text, NO_VARIABLES);
    assert_eq!(listing(&world, &iara), EMPTY_LISTING);
    assert!(iara.slot.end().variables.accepted.is_empty());
}

#[test]
fn set_cria_a_variavel_e_o_slot_leva_o_que_foi_aceito_para_o_orquestrador() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);

    open_exchange(&iara);
    let outcome = set(&world, &iara, "contagem", json!(3));
    let effects = iara.slot.end().variables;

    assert!(!outcome.is_error, "{}", outcome.text);
    assert_eq!(outcome.text, "Variável contagem registrada.");
    assert_eq!(
        effects.accepted,
        [Change::Set {
            name: "contagem".to_string(),
            value: VarValue::Number(3),
            share: false,
        }]
    );
    assert!(effects.created_variable);
    assert!(!effects.changed_value);
    assert_eq!(listing(&world, &iara), "- contagem (número, privada) = 3");
}

#[test]
fn texto_numerico_vira_numero_e_a_mudanca_de_valor_marca_changed_value() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);

    open_exchange(&iara);
    set(&world, &iara, "n", json!(1));
    iara.slot.end();
    open_exchange(&iara);
    let outcome = set(&world, &iara, "n", json!("2"));
    let effects = iara.slot.end().variables;

    assert!(!outcome.is_error, "{}", outcome.text);
    assert!(!effects.created_variable);
    assert!(effects.changed_value);
    assert_eq!(listing(&world, &iara), "- n (número, privada) = 2");
}

#[test]
fn recusa_volta_como_is_error_nao_grava_e_nao_entra_nos_efeitos() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    open_exchange(&iara);
    set(&world, &iara, "n", json!(1));
    iara.slot.end();

    open_exchange(&iara);
    let outcome = set(&world, &iara, "n", json!("abc"));
    let effects = iara.slot.end().variables;

    assert!(outcome.is_error);
    assert_eq!(
        outcome.text,
        "Variável recusada: a variável n é número; o valor enviado é texto."
    );
    assert!(effects.accepted.is_empty());
    assert!(!effects.created_variable && !effects.changed_value);
    assert_eq!(listing(&world, &iara), "- n (número, privada) = 1");
}

#[test]
fn cada_chamada_grava_sozinha_e_uma_recusa_depois_nao_desfaz_a_anterior() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);

    open_exchange(&iara);
    let first = set(&world, &iara, "a", json!("um"));
    let second = set(&world, &iara, "Inválido", json!("dois"));
    let effects = iara.slot.end().variables;

    assert!(!first.is_error);
    assert!(second.is_error);
    assert_eq!(effects.accepted.len(), 1);
    assert_eq!(listing(&world, &iara), "- a (texto, privada) = um");
}

#[test]
fn variavel_privada_nao_aparece_nem_aceita_escrita_de_outro_agente_ate_ser_compartilhada() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    let davi = world.agent("davi", "Davi", project);
    open_exchange(&iara);
    open_exchange(&davi);
    set(&world, &iara, "segredo", json!("x"));

    let hidden = listing(&world, &davi);
    let write = set(&world, &davi, "segredo", json!("y"));
    let shared = world.call(
        &iara,
        "set_variable",
        json!({ "name": "segredo", "op": "set", "value": "z", "share": true }),
    );

    assert_eq!(hidden, EMPTY_LISTING);
    assert!(write.is_error);
    assert!(write.text.contains("é privada de Iara"), "{}", write.text);
    assert!(!shared.is_error, "{}", shared.text);
    assert_eq!(
        listing(&world, &davi),
        "- segredo (texto, compartilhada) = z"
    );
}

#[test]
fn append_cria_a_lista_acrescenta_itens_e_aceita_compartilhar() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    let davi = world.agent("davi", "Davi", project);
    open_exchange(&iara);

    world.call(
        &iara,
        "set_variable",
        json!({ "name": "animais", "op": "append", "value": "gato" }),
    );
    let second = world.call(
        &iara,
        "set_variable",
        json!({ "name": "animais", "op": "append", "value": "cão", "share": true }),
    );

    assert!(!second.is_error, "{}", second.text);
    assert_eq!(
        listing(&world, &davi),
        "- animais (lista, compartilhada) = gato, cão"
    );
}

#[test]
fn operacao_e_valores_invalidos_sao_recusados_sem_gravar() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    open_exchange(&iara);
    let call = |op: &str, value: Value| {
        world.call(
            &iara,
            "set_variable",
            json!({ "name": "v", "op": op, "value": value }),
        )
    };

    let cases = [
        (
            call("apagar", json!("x")),
            "Operação inválida. Use set ou append.",
        ),
        (call("set", json!("   ")), "O valor não pode ser vazio."),
        (
            call("set", json!(1.5)),
            "O valor numérico precisa ser um inteiro.",
        ),
        (
            call("set", json!(["a"])),
            "O valor precisa ser um texto ou um número inteiro.",
        ),
        (call("append", json!("")), "O valor não pode ser vazio."),
    ];

    for (outcome, text) in cases {
        assert!(outcome.is_error, "{text}");
        assert_eq!(outcome.text, text);
    }
    assert_eq!(listing(&world, &iara), EMPTY_LISTING);
}

#[test]
fn variaveis_de_um_projeto_nao_aparecem_no_outro() {
    let world = world();
    let first = world.project("a");
    let second = world.project("b");
    let in_first = world.agent("iara", "Iara", first);
    let in_second = world.agent("iara", "Iara", second);
    open_exchange(&in_first);

    set(&world, &in_first, "v", json!("x"));

    assert_eq!(listing(&world, &in_first), "- v (texto, privada) = x");
    assert_eq!(listing(&world, &in_second), EMPTY_LISTING);
}

#[test]
fn project_id_e_agent_nos_argumentos_sao_ignorados() {
    let world = world();
    let first = world.project("a");
    let second = world.project("b");
    let iara = world.agent("iara", "Iara", first);
    let other = world.agent("iara", "Iara", second);
    open_exchange(&iara);

    world.call(
        &iara,
        "set_variable",
        json!({ "name": "v", "op": "set", "value": "x", "project_id": second, "agent": "davi" }),
    );

    assert_eq!(listing(&world, &iara), "- v (texto, privada) = x");
    assert_eq!(listing(&world, &other), EMPTY_LISTING);
}

#[test]
fn absorb_soma_os_efeitos_de_variavel_de_varias_tentativas() {
    let accepted = |name: &str| Change::Set {
        name: name.to_string(),
        value: VarValue::Number(1),
        share: false,
    };
    let mut collected = ExchangeOutcome::default();
    collected.variables.accepted.push(accepted("a"));
    collected.variables.created_variable = true;
    let mut next = ExchangeOutcome::default();
    next.variables.accepted.push(accepted("b"));
    next.variables.changed_value = true;

    collected.absorb(next);

    assert_eq!(collected.variables.accepted.len(), 2);
    assert!(collected.variables.created_variable);
    assert!(collected.variables.changed_value);
}
