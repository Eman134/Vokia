use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

use serde_json::{json, Value};
use tokio::net::TcpStream;

use super::support::*;
use super::*;

const OVER_LIMIT_BYTES: usize = 64 * 1024 + 1;
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

async fn started() -> (McpServer, u16, Token) {
    let server = McpServer::start().await.expect("o servidor deveria abrir");
    let port = port_of(&server);
    let token = server
        .register(binding("axiom", "Axiom"))
        .expect("o token deveria ser gerado");
    (server, port, token)
}

#[test]
fn bind_em_porta_zero_devolve_porta_maior_que_zero_e_url_em_loopback() {
    run(async {
        let (server, port, _) = started().await;

        assert!(port > 0);
        assert_eq!(server.url(), format!("http://127.0.0.1:{port}/mcp"));
    });
}

#[test]
fn servidor_nao_atende_fora_do_loopback() {
    run(async {
        let (server, port, _) = started().await;
        let probe = UdpSocket::bind("0.0.0.0:0").expect("socket de sondagem");
        let lan = probe
            .connect("192.0.2.1:9")
            .ok()
            .and_then(|()| probe.local_addr().ok())
            .map(|address| address.ip());
        let ipv6_loopback = SocketAddr::from((IpAddr::from(std::net::Ipv6Addr::LOCALHOST), port));

        let ipv6 = tokio::time::timeout(IO_TIMEOUT, TcpStream::connect(ipv6_loopback))
            .await
            .expect("conectar demorou");

        assert!(ipv6.is_err(), "o servidor não pode ouvir em ::1");
        if let Some(ip) =
            lan.filter(|ip| !ip.is_loopback() && *ip != IpAddr::V4(Ipv4Addr::UNSPECIFIED))
        {
            let external =
                tokio::time::timeout(IO_TIMEOUT, TcpStream::connect(SocketAddr::new(ip, port)))
                    .await
                    .expect("conectar demorou");
            assert!(external.is_err(), "o servidor não pode ouvir em {ip}");
        }
        drop(server);
    });
}

#[test]
fn sem_cabecalho_authorization_responde_401() {
    run(async {
        let (_server, port, token) = started().await;

        let reply = send(
            port,
            &Request::post(port, &token, rpc(1, "ping")).without_authorization(),
        )
        .await;

        assert_eq!(reply.status, 401);
    });
}

#[test]
fn token_errado_em_branco_ou_com_bearer_em_caixa_errada_responde_401() {
    run(async {
        let (_server, port, token) = started().await;
        let valid = token.as_str().to_string();
        let wrong_last = format!(
            "{}{}",
            &valid[..valid.len() - 1],
            if valid.ends_with('0') { '1' } else { '0' }
        );
        let cases = [
            ("token errado", format!("Bearer {wrong_last}")),
            ("token em branco", "Bearer ".to_string()),
            ("sem esquema", valid.clone()),
            ("bearer minúsculo", format!("bearer {valid}")),
            ("BEARER maiúsculo", format!("BEARER {valid}")),
        ];

        for (label, authorization) in cases {
            let reply = send(
                port,
                &Request::post(port, &token, rpc(1, "ping")).raw_authorization(&authorization),
            )
            .await;

            assert_eq!(reply.status, 401, "{label}");
        }
    });
}

#[test]
fn token_revogado_responde_401_e_antes_da_revogacao_respondia_200() {
    run(async {
        let (server, port, token) = started().await;
        let before = send(port, &Request::post(port, &token, rpc(1, "ping"))).await;

        server.revoke(&token);
        let after = send(port, &Request::post(port, &token, rpc(2, "ping"))).await;

        assert_eq!(before.status, 200);
        assert_eq!(after.status, 401);
    });
}

#[test]
fn dois_tokens_de_agentes_diferentes_nao_se_cruzam() {
    run(async {
        let (server, port, axiom) = started().await;
        let vexa = server
            .register(binding("vexa", "Vexa"))
            .expect("token do Vexa");

        assert_ne!(axiom.as_str(), vexa.as_str());
        server.revoke(&axiom);
        let revoked = send(port, &Request::post(port, &axiom, rpc(1, "ping"))).await;
        let kept = send(port, &Request::post(port, &vexa, rpc(2, "ping"))).await;

        assert_eq!(revoked.status, 401);
        assert_eq!(kept.status, 200);
        assert_eq!(kept.json()["id"], json!(2));
    });
}

#[test]
fn duas_chamadas_concorrentes_de_agentes_diferentes_recebem_cada_uma_a_sua_resposta() {
    run(async {
        let (server, port, axiom) = started().await;
        let vexa = server
            .register(binding("vexa", "Vexa"))
            .expect("token do Vexa");
        let tasks: Vec<_> = (0..20)
            .map(|index| {
                let (token, id) = if index % 2 == 0 {
                    (axiom.clone(), 1000 + index)
                } else {
                    (vexa.clone(), 2000 + index)
                };
                tauri::async_runtime::spawn(async move {
                    let reply = send(port, &Request::post(port, &token, rpc(id, "ping"))).await;
                    (id, reply)
                })
            })
            .collect();

        for task in tasks {
            let (id, reply) = task.await.expect("a tarefa não deveria falhar");
            assert_eq!(reply.status, 200);
            assert_eq!(reply.json()["id"], json!(id));
        }
    });
}

#[test]
fn host_errado_responde_403_mesmo_com_token_valido() {
    run(async {
        let (_server, port, token) = started().await;

        for host in [
            "localhost",
            "evil.example",
            &format!("127.0.0.1:{}", port.wrapping_add(1)),
            "127.0.0.1",
        ] {
            let reply = send(
                port,
                &Request::post(port, &token, rpc(1, "ping")).with_host(host),
            )
            .await;

            assert_eq!(reply.status, 403, "Host {host}");
        }
    });
}

#[test]
fn origin_presente_responde_403_mesmo_com_token_valido() {
    run(async {
        let (_server, port, token) = started().await;

        let reply = send(
            port,
            &Request::post(port, &token, rpc(1, "ping")).with_origin("http://evil.example"),
        )
        .await;

        assert_eq!(reply.status, 403);
    });
}

#[test]
fn initialize_devolve_versao_capacidade_de_ferramentas_e_nome_vokia() {
    run(async {
        let (_server, port, token) = started().await;
        let body = json!({ "jsonrpc": "2.0", "id": 7, "method": "initialize", "params": { "protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": { "name": "teste", "version": "0" } } });

        let reply = send(port, &Request::post(port, &token, body)).await;

        let json = reply.json();
        assert_eq!(reply.status, 200);
        assert_eq!(json["jsonrpc"], json!("2.0"));
        assert_eq!(json["id"], json!(7));
        assert_eq!(json["result"]["protocolVersion"], json!("2025-03-26"));
        assert!(json["result"]["capabilities"]["tools"].is_object());
        assert_eq!(json["result"]["serverInfo"]["name"], json!("vokia"));
    });
}

#[test]
fn initialize_com_versao_desconhecida_responde_a_mais_recente_suportada() {
    run(async {
        let (_server, port, token) = started().await;
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "1999-01-01" } });

        let reply = send(port, &Request::post(port, &token, body)).await;

        assert_eq!(
            reply.json()["result"]["protocolVersion"],
            json!("2025-06-18")
        );
    });
}

#[test]
fn notifications_initialized_responde_204_sem_corpo() {
    run(async {
        let (_server, port, token) = started().await;
        let body = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });

        let reply = send(port, &Request::post(port, &token, body)).await;

        assert_eq!(reply.status, 204);
        assert_eq!(reply.body, "");
    });
}

#[test]
fn tools_list_responde_json_rpc_valido_com_lista_vazia_nesta_etapa() {
    run(async {
        let (_server, port, token) = started().await;

        let reply = send(port, &Request::post(port, &token, rpc(3, "tools/list"))).await;

        assert_eq!(reply.status, 200);
        assert_eq!(
            reply.json(),
            json!({ "jsonrpc": "2.0", "id": 3, "result": { "tools": [] } })
        );
    });
}

#[test]
fn get_em_mcp_responde_405() {
    run(async {
        let (_server, port, token) = started().await;

        let reply = send(
            port,
            &Request::post(port, &token, rpc(1, "ping"))
                .with_method("GET")
                .with_body(Vec::new()),
        )
        .await;

        assert_eq!(reply.status, 405);
    });
}

#[test]
fn server_discover_responde_menos_32601_e_o_servidor_segue_atendendo_initialize() {
    run(async {
        let (_server, port, token) = started().await;

        let discover = send(
            port,
            &Request::post(port, &token, rpc(1, "server/discover")),
        )
        .await;
        let initialize = send(port, &Request::post(port, &token, rpc(2, "initialize"))).await;

        assert_eq!(discover.status, 200);
        assert_eq!(discover.error_code(), METHOD_NOT_FOUND);
        assert_eq!(discover.json()["id"], json!(1));
        assert_eq!(
            initialize.json()["result"]["serverInfo"]["name"],
            json!("vokia")
        );
    });
}

#[test]
fn metodo_desconhecido_responde_menos_32601_com_o_id_da_chamada() {
    run(async {
        let (_server, port, token) = started().await;

        let reply = send(port, &Request::post(port, &token, rpc(9, "resources/list"))).await;

        assert_eq!(reply.error_code(), METHOD_NOT_FOUND);
        assert_eq!(reply.json()["id"], json!(9));
    });
}

#[test]
fn requisicao_sem_o_campo_method_responde_menos_32600() {
    run(async {
        let (_server, port, token) = started().await;
        let body = json!({ "jsonrpc": "2.0", "id": 4 });

        let reply = send(port, &Request::post(port, &token, body)).await;

        assert_eq!(reply.error_code(), INVALID_REQUEST);
    });
}

#[test]
fn json_invalido_responde_menos_32700_com_id_nulo() {
    run(async {
        let (_server, port, token) = started().await;

        let reply = send(
            port,
            &Request::post(port, &token, rpc(1, "ping")).with_body(b"{ isso nao e json".to_vec()),
        )
        .await;

        assert_eq!(reply.status, 200);
        assert_eq!(reply.error_code(), PARSE_ERROR);
        assert_eq!(reply.json()["id"], Value::Null);
    });
}

#[test]
fn tools_call_de_ferramenta_desconhecida_responde_menos_32602_citando_o_nome() {
    run(async {
        let (_server, port, token) = started().await;
        let body = json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": "apagar_tudo", "arguments": {} } });

        let reply = send(port, &Request::post(port, &token, body)).await;

        assert_eq!(reply.error_code(), INVALID_PARAMS);
        assert_eq!(reply.json()["id"], json!(5));
        assert!(reply.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("apagar_tudo"));
    });
}

#[test]
fn tools_call_sem_nome_responde_menos_32602() {
    run(async {
        let (_server, port, token) = started().await;
        let body = json!({ "jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {} });

        let reply = send(port, &Request::post(port, &token, body)).await;

        assert_eq!(reply.error_code(), INVALID_PARAMS);
    });
}

#[test]
fn corpo_acima_de_64_kb_responde_413_e_no_limite_exato_responde_200() {
    run(async {
        let (_server, port, token) = started().await;
        let padding = |total: usize| {
            let base =
                json!({ "jsonrpc": "2.0", "id": 1, "method": "ping", "params": { "x": "" } })
                    .to_string();
            let fill = total - base.len();
            json!({ "jsonrpc": "2.0", "id": 1, "method": "ping", "params": { "x": "a".repeat(fill) } }).to_string().into_bytes()
        };

        let over = send(
            port,
            &Request::post(port, &token, rpc(1, "ping")).with_body(padding(OVER_LIMIT_BYTES)),
        )
        .await;
        let at_limit = send(
            port,
            &Request::post(port, &token, rpc(1, "ping")).with_body(padding(OVER_LIMIT_BYTES - 1)),
        )
        .await;

        assert_eq!(over.status, 413);
        assert_eq!(at_limit.status, 200);
    });
}

#[test]
fn vokia_cli_lista_tools_de_memoria_e_grafo_via_http_real() {
    run(async {
        let (server, _port, token) = started().await;
        server.install_test_tools();
        let bin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("debug")
            .join(if cfg!(windows) { "vokia-cli.exe" } else { "vokia-cli" });
        assert!(bin.exists(), "vokia-cli binário deveria existir em {:?}", bin);

        let out = tokio::process::Command::new(&bin)
            .args(["tools", "--json"])
            .env("VOKIA_URL", server.url())
            .env("VOKIA_TOKEN", token.as_str())
            .output()
            .await
            .expect("vokia-cli deveria executar");
        assert!(
            out.status.success(),
            "vokia-cli falhou: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("memory_graph_query"), "faltou memory_graph_query: {stdout}");
        assert!(stdout.contains("memory_graph_save"), "faltou memory_graph_save: {stdout}");
        assert!(stdout.contains("create_card"), "faltou create_card: {stdout}");
        assert!(stdout.contains("save_item_plan"), "faltou save_item_plan: {stdout}");
        assert!(stdout.contains("move_card"), "faltou move_card: {stdout}");
    });
}
