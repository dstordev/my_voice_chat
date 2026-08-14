use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, Stream, StreamConfig};
use ringbuf::traits::*;

use crate::codec::FRAME_SIZE;
use crate::resampler::{InputResampler, OutputResampler};

/// Структура-конфигурация, чтобы удобнее было передавать настройки
pub struct AudioSettings {
    pub target_sample_rate: u32,
    pub ring_buffer_capacity: usize,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            target_sample_rate: 48000,  // 48000 Гц
            ring_buffer_capacity: 4096, // Емкость кольцевого буфера
        }
    }
}

fn get_best_input_config(device: &Device, target_sr: u32) -> Result<StreamConfig> {
    let default_cfg = device.default_input_config()?;
    let default_sr = default_cfg.sample_rate();
    let target_channels = default_cfg.channels();

    // Если устройство Bluetooth / гарнитура с низкой частотой (16кГц / 24кГц), не форсируем 48кГц, так как CoreAudio/драйвер упадет с UnsupportedConfig
    if default_sr <= 24000 {
        let mut stream_cfg = default_cfg.config();
        stream_cfg.buffer_size = cpal::BufferSize::Default;
        return Ok(stream_cfg);
    }

    if let Ok(configs) = device.supported_input_configs() {
        for config in configs {
            // Ищем конфиг, который сохраняет родное количество каналов микрофона и поддерживает 48000 Гц
            if config.channels() == target_channels
                && config.min_sample_rate() <= target_sr
                && target_sr <= config.max_sample_rate()
            {
                let mut stream_cfg = config.with_sample_rate(target_sr).config();
                stream_cfg.buffer_size = cpal::BufferSize::Default;
                return Ok(stream_cfg);
            }
        }
    }

    // Если 48000 Гц с такими каналами не найдены (например, AirPods), берем дефолтный конфиг
    let mut stream_cfg = default_cfg.config();
    stream_cfg.buffer_size = cpal::BufferSize::Default;
    Ok(stream_cfg)
}

fn get_best_output_config(device: &Device, target_sr: u32) -> Result<StreamConfig> {
    let default_cfg = device.default_output_config()?;
    let target_channels = default_cfg.channels();

    if let Ok(configs) = device.supported_output_configs() {
        for config in configs {
            // Ищем конфиг, который сохраняет родное количество каналов динамиков (стерео) и поддерживает 48000 Гц
            if config.channels() == target_channels
                && config.min_sample_rate() <= target_sr
                && target_sr <= config.max_sample_rate()
            {
                let mut stream_cfg = config.with_sample_rate(target_sr).config();
                stream_cfg.buffer_size = cpal::BufferSize::Default;
                return Ok(stream_cfg);
            }
        }
    }

    // Если 48000 Гц не найдены, берем дефолтный конфиг
    let mut stream_cfg = default_cfg.config();
    stream_cfg.buffer_size = cpal::BufferSize::Default;
    Ok(stream_cfg)
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

    let input_config = get_best_input_config(&input_device, settings.target_sample_rate)?;
    let output_config = get_best_output_config(&output_device, settings.target_sample_rate)?;

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
    let sample_rate = input_config.sample_rate;
    let needs_resample = sample_rate != 48000;
    let mut resampler = InputResampler::new(sample_rate, 48000);

    let err_fn = |err: cpal::Error| eprintln!("🔴 | Ошибка в аудиопотоке: {}", err);

    let input_data_fn = move |chunk: &[f32], _: &cpal::InputCallbackInfo| {
        // Звуковая карта отдает `chunk` для произвольного использования
        // Делим вход на кадры и берем только 1-й канал
        for frame in chunk.chunks(in_channels) {
            let mono_sample = downmix(in_channels as f32, frame);
            if needs_resample {
                resampler.process(mono_sample, |s| {
                    let _ = producer.try_push(s);
                });
            } else {
                let _ = producer.try_push(mono_sample);
            }
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
    let sample_rate = output_config.sample_rate;
    let needs_resample = sample_rate != 48000;
    let mut resampler = OutputResampler::new(48000, sample_rate);

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
            let sample = if needs_resample {
                resampler.next_sample(|| consumer.try_pop().unwrap_or(0.0))
            } else {
                consumer.try_pop().unwrap_or(0.0)
            };
            // Заполняем фрейм одним моно-сэмплом
            frame.fill(sample);
        }
    };

    let output_stream = output_device
        .build_output_stream(output_config, output_data_fn, err_fn, None)
        .expect("🔴 | Не удалось собрать поток вывода.");

    Ok(output_stream)
}

fn downmix(input_channels: f32, frame: &[f32]) -> f32 {
    // Складываем значения всех каналов в фрейме и делим на их кол-во.
    // Таким образом [0.5, -0.1] (стерео) превратится в 0.2 (моно).
    let sum: f32 = frame.iter().sum();
    return sum / input_channels;
}
