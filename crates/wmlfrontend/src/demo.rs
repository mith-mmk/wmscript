use wmltoolchain::{GameAsset, GameProject};

pub fn build_image_audio_demo_project() -> GameProject {
    GameProject::new(
        "image-audio-demo",
        "samples/imageaudio/main.wml",
        include_str!("../../../samples/imageaudio/main.wml"),
    )
    .push_asset(GameAsset::image(
        "scene/background",
        10,
        100,
        include_bytes!("../../../samples/audio_and_images/sample01.jpg").to_vec(),
    ))
    .push_asset(GameAsset::image(
        "scene/foreground",
        11,
        101,
        include_bytes!("../../../samples/audio_and_images/sample02.jpg").to_vec(),
    ))
    .push_asset(GameAsset::audio("bgm/loop", 12, 200, make_demo_wav()))
}

fn make_demo_wav() -> Vec<u8> {
    let sample_rate = 22_050u32;
    let duration_ms = 450u32;
    let total_samples = (sample_rate as u64 * duration_ms as u64 / 1000) as usize;
    let mut data = Vec::with_capacity(44 + total_samples * 2);

    let data_bytes = (total_samples * 2) as u32;
    let riff_size = 36 + data_bytes;
    let byte_rate = sample_rate * 2;
    let block_align = 2u16;
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&riff_size.to_le_bytes());
    data.extend_from_slice(b"WAVE");
    data.extend_from_slice(b"fmt ");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&sample_rate.to_le_bytes());
    data.extend_from_slice(&byte_rate.to_le_bytes());
    data.extend_from_slice(&block_align.to_le_bytes());
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(b"data");
    data.extend_from_slice(&data_bytes.to_le_bytes());

    let frequency = 440.0f32;
    let amplitude = 0.30f32;
    for sample in 0..total_samples {
        let t = sample as f32 / sample_rate as f32;
        let envelope = if t < 0.02 {
            t / 0.02
        } else if t > (duration_ms as f32 / 1000.0) - 0.04 {
            ((duration_ms as f32 / 1000.0) - t) / 0.04
        } else {
            1.0
        }
        .clamp(0.0, 1.0);
        let wave = (t * frequency * std::f32::consts::TAU).sin();
        let value = (wave * amplitude * envelope * i16::MAX as f32) as i16;
        data.extend_from_slice(&value.to_le_bytes());
    }

    data
}
