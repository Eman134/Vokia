use std::time::Duration;

use tokio::net::TcpStream;
use tokio::time::timeout;

const CONNECTIVITY_HOST: &str = "api.anthropic.com";
const CONNECTIVITY_PORT: u16 = 443;
const CONNECTIVITY_TIMEOUT: Duration = Duration::from_secs(3);

pub(crate) async fn is_reachable() -> bool {
    let connection = TcpStream::connect((CONNECTIVITY_HOST, CONNECTIVITY_PORT));
    matches!(timeout(CONNECTIVITY_TIMEOUT, connection).await, Ok(Ok(_)))
}
