use anyhow::Result;
use iroh::{Endpoint, endpoint::presets};

const ALPN: &[u8] = b"my-voice-chat/v1";

#[tokio::main]
async fn main() -> Result<()> {
    let endpoint = Endpoint::builder(presets::N0)
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await?;

    println!("Ваш EndpointId: {}", endpoint.addr().id);
    println!("Ждем входящие P2P подключения...");

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

        println!(
            "🟢 | Новое P2P подключение от узла: {:?}",
            connection.remote_id()
        );

        tokio::spawn(async move {
            while let Ok(bytes) = connection.read_datagram().await {
                let msg = String::from_utf8_lossy(&bytes);
                println!("📩 | Получена датаграмма: {msg}");
            }
            println!("🔴 | Соединение разорвано");
        });
    }

    Ok(())
}
