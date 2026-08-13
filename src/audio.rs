use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, Stream, StreamConfig};
use ringbuf::traits::*;

use crate::codec::FRAME_SIZE;

/// Структура-конфигурация, чтобы удобнее было передавать настройки
pub struct AudioSettings {
    pub target_sample_rate: u32,
    pub input_channels: u16,
    pub ring_buffer_capacity: usize,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            target_sample_rate: 48000,  // 48000 Гц
            input_channels: 1,          // 1 канал (Моно)
            ring_buffer_capacity: 4096, // Емкость кольцевого буфера
        }
    }
}

/// Получаем дефолтные устройства и подготавливаем их конфиги
pub fn setup_devices(
    settings: &AudioSettings,
) -> Result<(Device, StreamConfig, Device, StreamConfig)> {
    let host = cpal::default_host();
    println!("ℹ️ | Аудио-хост: {:?}", host.id());

    let output_device = host
        .default_output_device()
        .expect("🔴 | Не удалось получить динамики.");

    let input_device = host
        .default_input_device()
        .expect("🔴 | Не удалось получить микрофон.");

    let mut input_config = input_device.default_input_config()?.config();
    let mut output_config = output_device.default_output_config()?.config();

    input_config.channels = settings.input_channels;

    // Принудительно ставим одинаковую частоту для обеих сторон
    input_config.sample_rate = settings.target_sample_rate;
    output_config.sample_rate = settings.target_sample_rate;

    // Даем PipeWire самому выбирать размер буфера
    input_config.buffer_size = cpal::BufferSize::Default;
    output_config.buffer_size = cpal::BufferSize::Default;

    println!("ℹ️ | Конфиг микрофона: {:?}", input_config);
    println!("ℹ️ | Конфиг динамиков: {:?}", output_config);

    Ok((input_device, input_config, output_device, output_config))
}

/// Функция сборки потока ВВОДА (микрофон)
pub fn create_input_stream(
    input_device: &Device,
    input_config: StreamConfig,
    mut producer: impl Producer<Item = f32> + Send + 'static,
) -> Result<Stream> {
    let in_channels = input_config.channels as usize;

    let err_fn = |err: cpal::Error| eprintln!("🔴 | Ошибка в аудиопотоке: {}", err);

    let input_data_fn = move |chunk: &[f32], _: &cpal::InputCallbackInfo| {
        // Звуковая карта отдает `chunk` для произвольного использования
        // Делим вход на кадры и берем только 1-й канал
        for frame in chunk.chunks(in_channels) {
            let _ = producer.try_push(frame[0]);
        }
    };

    let input_stream = input_device
        .build_input_stream(input_config, input_data_fn, err_fn, None)
        .expect("🔴 | Не удалось собрать поток ввода.");

    Ok(input_stream)
}

/// Функция сборки потока ВЫВОДА (динамики)
pub fn create_output_stream(
    output_device: &Device,
    output_config: StreamConfig,
    mut consumer: impl Consumer<Item = f32> + Send + 'static,
) -> Result<Stream> {
    let out_channels = output_config.channels as usize;

    let err_fn = |err: cpal::Error| eprintln!("🔴 | Ошибка в аудиопотоке: {}", err);

    // Накапливаем 3 кадра Opus (~60 мс звука) для защиты от сетевых задержек
    let target_buffer = FRAME_SIZE * 3;
    let mut is_buffering = true;

    let output_data_fn = move |chunk: &mut [f32], _: &cpal::OutputCallbackInfo| {
        let occupied = consumer.occupied_len();

        // 1. Если мы накопили мало сэмплов — отдаем тишину и ждем
        if is_buffering {
            if occupied >= target_buffer {
                is_buffering = false;
            } else {
                chunk.fill(0.0);
                return;
            }
        }

        // 2. Если буфер опустел полностью — снова включаем накопление
        if occupied == 0 {
            is_buffering = true;
            chunk.fill(0.0);
            return;
        }

        // Звуковая карта отдает `chunk`, куда просит записать звук
        for frame in chunk.chunks_mut(out_channels) {
            // `frame` тут стерео, поэтому `frame = [Л, П, Л, П, ...]`
            // Берем 1 моно-сэмпл
            let sample = consumer.try_pop().unwrap_or(0.0);
            // Заполняем фрейм одним моно-сэмплом
            frame.fill(sample);
        }
    };

    let output_stream = output_device
        .build_output_stream(output_config, output_data_fn, err_fn, None)
        .expect("🔴 | Не удалось собрать поток вывода.");

    Ok(output_stream)
}
