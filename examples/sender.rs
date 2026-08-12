use anyhow::Result;
use cpal::traits::StreamTrait;
use iroh::{Endpoint, EndpointId, endpoint::presets};
use my_voice_chat::audio;
use ringbuf::HeapRb;
use ringbuf::traits::*;
use std::io::{self, Write};
use std::time::Duration;

const ALPN: &[u8] = b"my-voice-chat/v1";

#[tokio::main]
async fn main() -> Result<()> {
    let settings = audio::AudioSettings::default();
    let (input_device, input_config, _, _) = audio::setup_devices(&settings)?;

    let ring = HeapRb::<f32>::new(settings.ring_buffer_capacity * 16);
    let (producer, mut consumer) = ring.split();

    let stream = audio::create_input_stream(&input_device, input_config, producer)?;
    stream.play()?;

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

    let mut interval = tokio::time::interval(Duration::from_millis(5));
    let mut buf = [0.0f32; 240];

    loop {
        interval.tick().await;
        if consumer.occupied_len() >= 240 {
            consumer.pop_slice(&mut buf);
            let bytes: Vec<u8> = buf.iter().flat_map(|s| s.to_le_bytes()).collect();
            if connection.send_datagram(bytes.into()).is_err() {
                break;
            }
        }
    }

    Ok(())
}
