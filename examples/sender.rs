use anyhow::Result;
use iroh::{Endpoint, EndpointId, endpoint::presets};
use std::io::{self, Write};

const ALPN: &[u8] = b"my-voice-chat/v1";

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 | Запуск Iroh Sender...");
    let endpoint = Endpoint::bind(presets::N0).await?;

    print!("🔑 | Вставьте EndpointId приемника: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    println!("⏳ | Установка P2P соединения...");
    let node_id: EndpointId = input.trim().parse()?;
    let connection = endpoint.connect(node_id, ALPN).await?;
    println!("🟢 | Успешно подключено!");

    println!("\n✍️ | Вводите текст для отправки:");

    for count in 1.. {
        print!("> ");
        io::stdout().flush()?;

        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            break;
        }

        let msg = format!("[Пакет #{count}] {}", line.trim());
        connection.send_datagram(msg.into())?;
        println!("📤 | Отправлено!");
    }

    Ok(())
}
