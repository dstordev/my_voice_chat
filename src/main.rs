use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::HeapRb;
use ringbuf::traits::*;
use std::io;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target_sample_rate = 48000; // 48000 Гц
    let input_channels = 1; // 1 канал (Моно)
    let ring_buffer_capacity = 1024; // Емкость кольцевого буфера

    let host = cpal::default_host();
    println!("ℹ️ | Аудио-хост: {:?}", host.id());

    let default_output_device = host
        .default_output_device()
        .expect("🔴 | Не удалось получить динамики.");

    let default_input_device = host
        .default_input_device()
        .expect("🔴 | Не удалось получить микрофон.");

    let mut input_config = default_input_device.default_input_config()?.config();
    let mut output_config = default_output_device.default_output_config()?.config();

    input_config.channels = input_channels;

    // Принудительно ставим одинаковую частоту 48000 Гц для обеих сторон
    input_config.sample_rate = target_sample_rate;
    output_config.sample_rate = target_sample_rate;

    // Даем PipeWire самому выбирать размер буфера
    input_config.buffer_size = cpal::BufferSize::Default;
    output_config.buffer_size = cpal::BufferSize::Default;

    println!("ℹ️ | Конфиг микрофона: {:?}", input_config);
    println!("ℹ️ | Конфиг динамиков: {:?}", output_config);

    let in_channels = input_config.channels as usize;
    let out_channels = output_config.channels as usize;

    let ring = HeapRb::<f32>::new(ring_buffer_capacity);
    let (mut producer, mut consumer) = ring.split();

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

    let err_fn = |err: cpal::Error| eprintln!("🔴 | Ошибка в аудиопотоке: {}", err);

    let input_stream = default_input_device
        .build_input_stream(input_config, input_data_fn, err_fn, None)
        .expect("🔴 | Не удалось собрать поток ввода.");

    let output_stream = default_output_device
        .build_output_stream(output_config, output_data_fn, err_fn, None)
        .expect("🔴 | Не удалось собрать поток вывода.");

    input_stream.play()?;
    output_stream.play()?;

    println!("🟢 | Звук с микрофона успешно идет к динамикам!");
    println!("Нажмите Enter, чтобы остановить...");
    let mut dummy = String::new();
    io::stdin().read_line(&mut dummy)?;

    Ok(())
}
