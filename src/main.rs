mod audio; // Подключаем наш новый модуль

use cpal::traits::StreamTrait;
use ringbuf::HeapRb;
use ringbuf::traits::*;
use std::io;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Берем дефолтные настройки
    let settings = audio::AudioSettings::default();

    // 2. Инициализируем устройства и их конфиги
    let (input_device, input_config, output_device, output_config) =
        audio::setup_devices(&settings)?;

    // 3. Создаем кольцевой буфер
    let ring = HeapRb::<f32>::new(settings.ring_buffer_capacity);
    let (producer, consumer) = ring.split();

    // 4. Собираем потоки, передавая им половинки буфера
    let input_stream = audio::create_input_stream(&input_device, input_config, producer)?;
    let output_stream = audio::create_output_stream(&output_device, output_config, consumer)?;

    // 5. Запускаем воспроизведение
    input_stream.play()?;
    output_stream.play()?;

    println!("🟢 | Звук с микрофона успешно идет к динамикам!");
    println!("Нажмите Enter, чтобы остановить...");
    let mut dummy = String::new();
    io::stdin().read_line(&mut dummy)?;

    Ok(())
}
