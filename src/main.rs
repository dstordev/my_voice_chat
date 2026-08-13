use anyhow::Result;
use cpal::traits::StreamTrait;
use iroh::EndpointId;
use my_voice_chat::{audio, codec, net};
use ringbuf::{HeapRb, traits::*};
use std::{
    io::{self, Write},
    time::Duration,
};

fn prompt(msg: &str) -> Result<String> {
    print!("{msg}");
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf.trim().to_owned())
}

#[tokio::main]
async fn main() -> Result<()> {
    let settings = audio::AudioSettings::default();
    let (in_dev, in_cfg, out_dev, out_cfg) = audio::setup_devices(&settings)?;

    let cap = settings.ring_buffer_capacity;
    let (in_prod, mut in_cons) = HeapRb::<f32>::new(cap).split();
    let (mut out_prod, out_cons) = HeapRb::<f32>::new(cap).split();

    let in_stream = audio::create_input_stream(&in_dev, in_cfg, in_prod)?;
    let out_stream = audio::create_output_stream(&out_dev, out_cfg, out_cons)?;
    in_stream.play()?;
    out_stream.play()?;

    let mode = prompt(
        "\nВыберите режим:\n1 - Ждать звонок (Host)\n2 - Позвонить (Client)\nВаш выбор [1/2]: ",
    )?;
    let endpoint = net::create_endpoint().await?;

    let conn = if mode == "1" {
        println!(
            "\n🔑 | Ваш EndpointId: {}\n⏳ | Ожидание звонка...",
            endpoint.addr().id
        );
        net::accept_connection(&endpoint).await?
    } else {
        let node_id: EndpointId = prompt("\n🔑 | Введите EndpointId собеседника: ")?.parse()?;
        println!("⏳ | Установка P2P соединения...");
        net::connect_to_peer(&endpoint, node_id).await?
    };

    println!("🟢 | Звонок соединен! Можно говорить (Opus сжатие включено)!");

    let recv_conn = conn.clone();
    let recv_task = tokio::spawn(async move {
        let Ok(mut decoder) = codec::AudioDecoder::new() else {
            eprintln!("🔴 | Ошибка создания Opus декодера");
            return;
        };
        while let Ok(bytes) = recv_conn.read_datagram().await {
            if let Ok(chunk) = decoder.decode(&bytes) {
                let _ = out_prod.push_slice(&chunk);
            }
        }
        println!("🔴 | Собеседник отключился");
    });

    let send_task = tokio::spawn(async move {
        let Ok(mut encoder) = codec::AudioEncoder::new() else {
            eprintln!("🔴 | Ошибка создания Opus кодера");
            return;
        };
        let mut interval = tokio::time::interval(Duration::from_millis(5));
        let mut chunk = [0.0f32; codec::FRAME_SIZE];

        loop {
            interval.tick().await;
            while in_cons.occupied_len() >= codec::FRAME_SIZE {
                in_cons.pop_slice(&mut chunk);
                if let Ok(bytes) = encoder.encode(&chunk) {
                    if conn.send_datagram(bytes.into()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let _ = tokio::join!(recv_task, send_task);
    Ok(())
}
