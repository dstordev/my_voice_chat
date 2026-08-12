use anyhow::Result;
use cpal::traits::StreamTrait;
use iroh::EndpointId;
use my_voice_chat::{audio, net};
use ringbuf::HeapRb;
use ringbuf::traits::*;
use std::io::{self, Write};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let settings = audio::AudioSettings::default();
    let (input_device, input_config, output_device, output_config) =
        audio::setup_devices(&settings)?;

    let in_ring = HeapRb::<f32>::new(settings.ring_buffer_capacity * 2);
    let (in_prod, mut in_cons) = in_ring.split();

    let out_ring = HeapRb::<f32>::new(settings.ring_buffer_capacity * 2);
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

    println!("🟢 | Звонок соединен! Можно говорить!");

    let recv_conn = connection.clone();
    let recv_handle = tokio::spawn(async move {
        while let Ok(bytes) = recv_conn.read_datagram().await {
            let samples: Vec<f32> = bytes
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                .collect();
            let _ = out_prod.push_slice(&samples);
        }
        println!("🔴 | Собеседник отключился");
    });

    let send_conn = connection;
    let send_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(5));
        let mut buf = [0.0f32; 240];

        loop {
            interval.tick().await;
            if in_cons.occupied_len() >= 240 {
                in_cons.pop_slice(&mut buf);
                let bytes: Vec<u8> = buf.iter().flat_map(|s| s.to_le_bytes()).collect();
                if send_conn.send_datagram(bytes.into()).is_err() {
                    break;
                }
            }
        }
    });

    let _ = tokio::join!(recv_handle, send_handle);

    Ok(())
}
