use super::*;

#[test]
fn time_chamado_time_nao_repete_a_palavra() {
    assert_eq!(created_for_team("Ember", "Time", "Axiom"), "Ember criou o card para o time (líder Axiom).");
    assert_eq!(created_for_team("Ember", " time ", "Axiom"), "Ember criou o card para o time (líder Axiom).");
}

#[test]
fn time_cujo_nome_ja_comeca_com_time_nao_ganha_outro() {
    assert_eq!(created_for_team("Ember", "Time Core", "Axiom"), "Ember criou o card para o Time Core (líder Axiom).");
}

#[test]
fn time_com_outro_nome_aparece_com_o_nome() {
    assert_eq!(created_for_team("Ember", "Core", "Axiom"), "Ember criou o card para o time Core (líder Axiom).");
    assert_eq!(created_for_team("Ember", "Timeline", "Axiom"), "Ember criou o card para o time Timeline (líder Axiom).");
}
