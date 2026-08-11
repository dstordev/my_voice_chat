use cpal::StreamConfig;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::HeapRb;
use ringbuf::traits::*; // КРИТИЧНО ДЛЯ ringbuf 0.5+: подключает методы push/pop/split
use std::io;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target_sample_rate = 48000; // 48000 Гц
    let target_channels = 1; // 1 канал (Моно)

    let host = cpal::default_host();
    println!("[Аудио-хост]: {:?}", host.id());

    let default_output_device = host
        .default_output_device()
        .expect("Не удалось получить дефолтное устройство вывода.");
    println!(
        "[Дефолтное устройство вывода]: {:?}",
        default_output_device.to_string()
    );

    let default_input_device = host
        .default_input_device()
        .expect("Не удалось получить дефолтное устройство ввода.");
    println!(
        "[Дефолтное устройство ввода]: {:?}",
        default_input_device.to_string()
    );

    // Порция, которую просим у звуковой карты за раз (256 = ~5.3 мс)
    let harware_buffer_size = 256;
    // Емкость кольцевого буфера с большим запасом, чтобы предотвратить переполнение
    let ring_buffer_capacity = 4096;

    let config = StreamConfig {
        channels: target_channels,
        sample_rate: target_sample_rate,
        buffer_size: cpal::BufferSize::Fixed(harware_buffer_size),
    };

    let ring = HeapRb::<f32>::new(ring_buffer_capacity);
    let (mut producer, mut consumer) = ring.split();

    // Предзаполнение тишиной
    for _ in 0..(harware_buffer_size * 2) {
        let _ = producer.try_push(0.0);
    }

    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        for &sample in data {
            let _ = producer.try_push(sample);
        }
    };

    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        for sample in data.iter_mut() {
            *sample = consumer.try_pop().unwrap_or(0.0);
        }
    };

    let err_fn = |err: cpal::Error| eprintln!("Ошибка в аудиопотоке: {}", err);

    let input_stream = default_input_device
        .build_input_stream(config, input_data_fn, err_fn, None)
        .expect("Незивестная ошибка построения потока ввода");

    let output_stream = default_output_device
        .build_output_stream(config, output_data_fn, err_fn, None)
        .expect("Незивестная ошибка построения потока вывода");

    input_stream.play()?;
    output_stream.play()?;

    println!("[+] Проброс звука успешно запущен!");
    println!("Нажмите Enter, чтобы остановить...");
    let mut dummy = String::new();
    io::stdin().read_line(&mut dummy)?;

    Ok(())
}
