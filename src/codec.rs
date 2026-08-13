use anyhow::Result;
use audiopus::coder::{Decoder, Encoder};
use audiopus::{Application, Bandwidth, Channels, SampleRate};

pub const FRAME_SIZE: usize = 960;

pub struct AudioEncoder {
    encoder: Encoder,
}

impl AudioEncoder {
    pub fn new() -> Result<Self> {
        let mut encoder = Encoder::new(SampleRate::Hz48000, Channels::Mono, Application::LowDelay)?;

        let _ = encoder.set_bandwidth(Bandwidth::Fullband);

        let _ = encoder.set_bitrate(audiopus::Bitrate::BitsPerSecond(64_000)); // 64 kbps
        let _ = encoder.set_inband_fec(true); // Включаем устойчивость к потерям пакетов (FEC)
        let _ = encoder.set_packet_loss_perc(10); // Рассчитываем примерно на 10% потерь сети

        Ok(Self { encoder })
    }

    pub fn encode(&mut self, pcm: &[f32]) -> Result<Vec<u8>> {
        let mut output = [0u8; 512];
        let len = self.encoder.encode_float(pcm, &mut output[..])?;
        Ok(output[..len].to_vec())
    }
}

pub struct AudioDecoder {
    decoder: Decoder,
}

impl AudioDecoder {
    pub fn new() -> Result<Self> {
        let decoder = Decoder::new(SampleRate::Hz48000, Channels::Mono)?;
        Ok(Self { decoder })
    }

    pub fn decode(&mut self, opus_data: &[u8]) -> Result<Vec<f32>> {
        let mut pcm_out = [0.0f32; FRAME_SIZE];
        let len = self
            .decoder
            .decode_float(Some(opus_data), &mut pcm_out[..], false)?;
        Ok(pcm_out[..len].to_vec())
    }
}
