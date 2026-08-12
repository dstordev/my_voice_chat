use anyhow::Result;
use iroh::endpoint::Connection;
use iroh::{Endpoint, EndpointId, endpoint::presets};

pub const ALPN: &[u8] = b"my-voice-chat/v1";

pub async fn create_endpoint() -> Result<Endpoint> {
    let endpoint = Endpoint::builder(presets::N0)
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await?;
    Ok(endpoint)
}

pub async fn accept_connection(endpoint: &Endpoint) -> Result<Connection> {
    while let Some(incoming) = endpoint.accept().await {
        let Ok(connecting) = incoming
            .accept()
            .map_err(|e| eprintln!("🔴 | Ошибка приема: {e}"))
        else {
            continue;
        };
        let Ok(connection) = connecting
            .await
            .map_err(|e| eprintln!("🔴 | Ошибка рукопожатия: {e}"))
        else {
            continue;
        };
        return Ok(connection);
    }
    anyhow::bail!("Приемник закрылся без подключений")
}

pub async fn connect_to_peer(endpoint: &Endpoint, node_id: EndpointId) -> Result<Connection> {
    let connection = endpoint.connect(node_id, ALPN).await?;
    Ok(connection)
}
