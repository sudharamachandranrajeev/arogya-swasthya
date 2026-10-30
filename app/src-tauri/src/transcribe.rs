//! Transcripts of recordings, so search finds a voice note or a recorded
//! appointment by its words. Uses [whisper.cpp](https://github.com/ggml-org/whisper.cpp)
//! with its multilingual base model, on this device. Desktop only for now.

use std::io::Cursor;
use std::path::Path;
use std::sync::{Arc, Mutex, Once};

use swasthya_core::extract::Text;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as AudioError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::text::ModelFile;

/// The transcript model (whisper.cpp base, multilingual, 5-bit), pinned to
/// one commit of its repository.
pub(crate) const MODEL: ModelFile = ModelFile {
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-base-q5_1.bin",
    name: "ggml-base-q5_1.bin",
    sha256: "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898",
    size: 59_707_625,
};
/// Whisper hears 16 kHz mono.
const RATE: u32 = 16_000;
/// Only the first half hour of a recording is transcribed.
const MAX_SECONDS: usize = 30 * 60;

static MODEL_SLOT: Mutex<Option<Arc<WhisperContext>>> = Mutex::new(None);
static QUIET: Once = Once::new();

/// The words spoken in a recording (audio, or the sound of a video).
pub(crate) fn text_of(models: &Path, bytes: Vec<u8>) -> Text {
    let Some(samples) = audio(bytes) else {
        return Text::Unsupported;
    };
    let Some(ctx) = context(models) else {
        return Text::NeedsOcr;
    };
    match transcribe(&ctx, &samples) {
        Some(t) => Text::Found(t),
        None => Text::Unsupported,
    }
}

/// The model, loaded once it is on this device.
fn context(dir: &Path) -> Option<Arc<WhisperContext>> {
    QUIET.call_once(whisper_rs::install_logging_hooks);
    let mut slot = MODEL_SLOT.lock().ok()?;
    if slot.is_none() {
        let path = dir.join(MODEL.name);
        let ctx =
            WhisperContext::new_with_params(path.to_str()?, WhisperContextParameters::default())
                .ok()?;
        *slot = Some(Arc::new(ctx));
    }
    slot.clone()
}

fn transcribe(ctx: &WhisperContext, samples: &[f32]) -> Option<String> {
    let mut state = ctx.create_state().ok()?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("auto"));
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_special(false);
    params.set_print_timestamps(false);
    state.full(params, samples).ok()?;
    let lines: Vec<String> = state
        .as_iter()
        .map(|seg| seg.to_str_lossy().map(|s| s.trim().to_string()))
        .collect::<Result<_, _>>()
        .ok()?;
    Some(
        lines
            .into_iter()
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// Decode a recording to 16 kHz mono, at most [`MAX_SECONDS`] long.
/// `None` if it holds no sound this can decode.
fn audio(bytes: Vec<u8>) -> Option<Vec<f32>> {
    let stream = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let mut format = symphonia::default::get_probe()
        .probe(
            &Hint::new(),
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .ok()?;
    let track = format.default_track(TrackType::Audio)?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(
            track.codec_params.as_ref()?.audio()?,
            &AudioDecoderOptions::default(),
        )
        .ok()?;
    let mut mono = Vec::new();
    let mut rate = 0;
    let mut frame = Vec::new();
    while let Ok(Some(packet)) = format.next_packet() {
        if packet.track_id != track_id {
            continue;
        }
        let buf = match decoder.decode(&packet) {
            Ok(b) => b,
            Err(AudioError::DecodeError(_)) => continue,
            Err(_) => break,
        };
        rate = buf.spec().rate();
        let channels = buf.spec().channels().count().max(1);
        frame.resize(buf.samples_interleaved(), 0.0f32);
        buf.copy_to_slice_interleaved(&mut frame);
        mono.extend(
            frame
                .chunks(channels)
                .map(|c| c.iter().sum::<f32>() / channels as f32),
        );
        if rate > 0 && mono.len() >= MAX_SECONDS * rate as usize {
            break;
        }
    }
    if rate == 0 || mono.is_empty() {
        return None;
    }
    mono.truncate(MAX_SECONDS * rate as usize);
    Some(resample(&mono, rate))
}

/// Change the sample rate to [`RATE`]: averaging when going down, so high
/// sounds do not fold back as noise, and joining the dots when going up.
fn resample(input: &[f32], from: u32) -> Vec<f32> {
    if from == RATE {
        return input.to_vec();
    }
    let step = from as f64 / RATE as f64;
    let len = (input.len() as f64 / step) as usize;
    (0..len)
        .map(|i| {
            let start = i as f64 * step;
            if step > 1.0 {
                let a = start as usize;
                let b = (((i + 1) as f64 * step) as usize).clamp(a + 1, input.len());
                input[a..b].iter().sum::<f32>() / (b - a) as f32
            } else {
                let a = start as usize;
                let t = (start - a as f64) as f32;
                let next = input.get(a + 1).copied().unwrap_or(input[a]);
                input[a] * (1.0 - t) + next * t
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A WAV file of `seconds` of a tone, made here.
    fn wav(rate: u32, channels: u16, seconds: f32) -> Vec<u8> {
        let frames = (rate as f32 * seconds) as usize;
        let data: Vec<u8> = (0..frames)
            .flat_map(|i| {
                let v = ((i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 16_000.0)
                    as i16;
                std::iter::repeat_n(v.to_le_bytes(), channels as usize).flatten()
            })
            .collect();
        let mut out = Vec::new();
        out.extend(b"RIFF");
        out.extend((36 + data.len() as u32).to_le_bytes());
        out.extend(b"WAVEfmt ");
        out.extend(16u32.to_le_bytes());
        out.extend(1u16.to_le_bytes());
        out.extend(channels.to_le_bytes());
        out.extend(rate.to_le_bytes());
        out.extend((rate * channels as u32 * 2).to_le_bytes());
        out.extend((channels * 2).to_le_bytes());
        out.extend(16u16.to_le_bytes());
        out.extend(b"data");
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(data);
        out
    }

    #[test]
    fn decodes_and_resamples_recordings() {
        for (rate, channels) in [(44_100, 2), (16_000, 1), (8_000, 1)] {
            let samples = audio(wav(rate, channels, 2.0)).unwrap();
            assert!(
                (samples.len() as i64 - 32_000).abs() < 50,
                "{rate} Hz: {}",
                samples.len()
            );
            let loudest = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!((0.4..0.6).contains(&loudest), "{rate} Hz: {loudest}");
        }
        assert!(audio(b"not a recording".to_vec()).is_none());
        assert_eq!(
            text_of(Path::new("/nowhere"), b"junk".to_vec()),
            Text::Unsupported
        );
    }

    /// Needs the model and a recording of speech:
    /// WHISPER_MODEL=/folder/with/the/model WHISPER_SAMPLE=/path/to/speech
    /// WHISPER_WORD=a-word-said-in-it.
    #[test]
    fn transcribes_speech_with_the_model() {
        let (Some(dir), Some(sample), Some(word)) = (
            std::env::var_os("WHISPER_MODEL"),
            std::env::var_os("WHISPER_SAMPLE"),
            std::env::var("WHISPER_WORD").ok(),
        ) else {
            eprintln!("WHISPER_MODEL/SAMPLE/WORD not set: skipping the transcript test");
            return;
        };
        let bytes = std::fs::read(sample).unwrap();
        let Text::Found(t) = text_of(Path::new(&dir), bytes) else {
            panic!("no transcript")
        };
        assert!(t.to_lowercase().contains(&word), "{t}");
    }
}
