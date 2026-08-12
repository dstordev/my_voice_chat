use anyhow::Result;
use cpal::traits::StreamTrait;
use iroh::EndpointId;
use my_voice_chat::{audio, codec, net};
use ringbuf::HeapRb;
use ringbuf::traits::*;
use std::io::{self, Write};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let settings = audio::AudioSettings::default();
    let (input_device, input_config, output_device, output_config) =
        audio::setup_devices(&settings)?;

    let in_ring = HeapRb::<f32>::new(settings.ring_buffer_capacity * 4);
    let (in_prod, mut in_cons) = in_ring.split();

    let out_ring = HeapRb::<f32>::new(settings.ring_buffer_capacity * 4);
    let (mut out_prod, out_cons) = out_ring.split();

    let input_stream = audio::create_input_stream(&input_device, input_config, in_prod)?;
    let output_stream = audio::create_output_stream(&output_device, output_config, out_cons)?;

    input_stream.play()?;
    output_stream.play()?;

    println!("\n Выберите режим:");
    println!("1 - Ждать входящий звонок (Host)");
    println!("2 - Позвонить другу (Client)");
    print!("Ваш выбор [1/2]: ");
    io::stdout().flush()?;

    let mut mode = String::new();
    io::stdin().read_line(&mut mode)?;

    let endpoint = net::create_endpoint().await?;

    let connection = if mode.trim() == "1" {
        println!("\n🔑 | Ваш EndpointId: {}", endpoint.addr().id);
        println!("⏳ | Ожидание входящего звонка...");
        net::accept_connection(&endpoint).await?
    } else {
        print!("\n🔑 | Введите EndpointId собеседника: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let node_id: EndpointId = input.trim().parse()?;

        println!("⏳ | Установка P2P соединения...");
        net::connect_to_peer(&endpoint, node_id).await?
    };

    println!("🟢 | Звонок соединен! Можно говорить (Opus сжатие включено)!");

    let recv_conn = connection.clone();
    let recv_handle = tokio::spawn(async move {
        let Ok(mut decoder) = codec::AudioDecoder::new() else {
            eprintln!("🔴 | Ошибка создания Opus декодера");
            return;
        };

        while let Ok(bytes) = recv_conn.read_datagram().await {
            if let Ok(samples) = decoder.decode(&bytes) {
                let _ = out_prod.push_slice(&samples);
            }
        }
        println!("🔴 | Собеседник отключился");
    });

    let send_conn = connection;
    let send_handle = tokio::spawn(async move {
        let Ok(mut encoder) = codec::AudioEncoder::new() else {
            eprintln!("🔴 | Ошибка создания Opus кодера");
            return;
        };

        let mut interval = tokio::time::interval(Duration::from_millis(5));
        let mut buf = [0.0f32; codec::FRAME_SIZE];

        loop {
            interval.tick().await;
            if in_cons.occupied_len() >= codec::FRAME_SIZE {
                in_cons.pop_slice(&mut buf);
                if let Ok(encoded_bytes) = encoder.encode(&buf) {
                    if send_conn.send_datagram(encoded_bytes.into()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let _ = tokio::join!(recv_handle, send_handle);

    Ok(())
}
