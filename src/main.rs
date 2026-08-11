use cpal::SampleRate;
use cpal::traits::{DeviceTrait, HostTrait};

fn main() {
    let target_sample_rate: SampleRate = 48000; // 48000 Гц
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

    let config = default_output_device
        .default_output_config()
        .expect("Не удалось получить дефолтную конфигурацию дефолтного устройства вывода");
    println!(
        "Дефолтная конфигурация дефолтного устройства вывода: {:?}",
        config
    );
}
