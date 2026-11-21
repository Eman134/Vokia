use serde::Serialize;

pub const KIND_PLAN: &str = "plano";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactKind {
    pub kind: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub template: &'static str,
}

// Mesma lista do CHECK de artifact_documents.kind; um teste confere que as duas batem.
pub const ARTIFACT_KINDS: &[ArtifactKind] = &[
    ArtifactKind {
        kind: "documento",
        label: "Documento",
        description: "Texto livre para registrar contexto, pesquisa ou anotações do projeto.",
        template: "## Contexto\n\n## Conteúdo\n",
    },
    ArtifactKind {
        kind: "especificacao",
        label: "Especificação",
        description: "O que será construído: objetivo, requisitos e critérios de aceite.",
        template: "## Objetivo\n\n## Requisitos\n\n## Critérios de aceite\n\n## Fora do escopo\n",
    },
    ArtifactKind {
        kind: "checklist",
        label: "Checklist",
        description: "Lista de itens marcáveis para acompanhar uma entrega.",
        template: "- [ ] Primeiro item\n",
    },
    ArtifactKind {
        kind: "arquitetura",
        label: "Arquitetura",
        description: "Visão geral dos componentes, das ligações entre eles e dos trade-offs.",
        template: "## Visão geral\n\n## Componentes\n\n## Trade-offs\n",
    },
    ArtifactKind {
        kind: "decisao",
        label: "Decisão",
        description: "Registro de uma decisão técnica com contexto e consequências.",
        template: "## Contexto\n\n## Decisão\n\n## Consequências\n",
    },
];

pub fn find_kind(kind: &str) -> Option<&'static ArtifactKind> {
    ARTIFACT_KINDS.iter().find(|candidate| candidate.kind == kind)
}
