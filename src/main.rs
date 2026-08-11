use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::HeapRb;
use ringbuf::traits::*; // КРИТИЧНО ДЛЯ ringbuf 0.5+: подключает методы push/pop/split
use std::io;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target_sample_rate = 48000; // 48000 Гц
    let input_channels = 1; // 1 канал (Моно)

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

    let mut input_config = default_input_device.default_input_config()?.config();
    let mut output_config = default_output_device.default_output_config()?.config();

    input_config.channels = input_channels;

    // Принудительно ставим одинаковую частоту 48000 Гц для обеих сторон
    input_config.sample_rate = target_sample_rate;
    output_config.sample_rate = target_sample_rate;

    // Даем PipeWire самому выбирать размер буфера
    input_config.buffer_size = cpal::BufferSize::Default;
    output_config.buffer_size = cpal::BufferSize::Default;

    println!("[Входной конфиг]: {:?}", input_config);
    println!("[Выходной конфиг]: {:?}", output_config);

    let in_channels = input_config.channels as usize;
    let out_channels = output_config.channels as usize;

    // Емкость кольцевого буфера с большим запасом, чтобы предотвратить переполнение
    let ring_buffer_capacity = 16384;

    let ring = HeapRb::<f32>::new(ring_buffer_capacity);
    let (mut producer, mut consumer) = ring.split();

    // Предзаполнение тишиной
    // Предзаполняем 2048 сэмплов (~42 мс тишины), чтобы динамики не голодали во время квантов PipeWire
    for _ in 0..2048 {
        let _ = producer.try_push(0.0);
    }

    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        // Делим вход на кадры и берем только 1-й канал
        for frame in data.chunks(in_channels) {
            let _ = producer.try_push(frame[0]);
        }
    };

    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        // Делим выход на кадры и берем РОВНО 1 моно-сэмпл, копируя его во все динамики (Л + П)
        for frame in data.chunks_mut(out_channels) {
            let sample = consumer.try_pop().unwrap_or(0.0);
            for channel_sample in frame.iter_mut() {
                *channel_sample = sample;
            }
        }
    };

    let err_fn = |err: cpal::Error| eprintln!("Ошибка в аудиопотоке: {}", err);

    let input_stream = default_input_device
        .build_input_stream(input_config, input_data_fn, err_fn, None)
        .expect("Незивестная ошибка построения потока ввода");

    let output_stream = default_output_device
        .build_output_stream(output_config, output_data_fn, err_fn, None)
        .expect("Незивестная ошибка построения потока вывода");

    input_stream.play()?;
    output_stream.play()?;

    println!("[+] Проброс звука успешно запущен!");
    println!("Нажмите Enter, чтобы остановить...");
    let mut dummy = String::new();
    io::stdin().read_line(&mut dummy)?;

    Ok(())
}
