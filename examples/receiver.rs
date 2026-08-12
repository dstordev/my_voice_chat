use anyhow::Result;
use cpal::traits::StreamTrait;
use iroh::{Endpoint, endpoint::presets};
use my_voice_chat::audio;
use ringbuf::HeapRb;
use ringbuf::traits::*;

const ALPN: &[u8] = b"my-voice-chat/v1";

#[tokio::main]
async fn main() -> Result<()> {
    let settings = audio::AudioSettings::default();
    let (_, _, output_device, output_config) = audio::setup_devices(&settings)?;

    let ring = HeapRb::<f32>::new(settings.ring_buffer_capacity * 16);
    let (producer, consumer) = ring.split();

    let stream = audio::create_output_stream(&output_device, output_config, consumer)?;
    stream.play()?;

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

        let mut producer = producer;
        let handle = tokio::spawn(async move {
            while let Ok(bytes) = connection.read_datagram().await {
                let samples: Vec<f32> = bytes
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                    .collect();
                let _ = producer.push_slice(&samples);
            }
            println!("🔴 | Соединение разорвано");
        });

        handle.await?;
        break;
    }

    Ok(())
}
