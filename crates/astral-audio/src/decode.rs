use anyhow::{Context, Result, ensure};
use std::io::Cursor;
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, formats::FormatOptions, io::MediaSourceStream,
    meta::MetadataOptions, probe::Hint,
};
pub struct Pcm {
    pub samples: Vec<f32>,
    pub rate: u32,
    pub channels: usize,
}
/// Decode without a subprocess. Bound both encoded and decoded memory before accepting a source.
pub fn decode(bytes: &[u8]) -> Result<Pcm> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= 64 * 1024 * 1024,
        "Audio source must be within 64 MiB"
    );
    let stream = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("Unsupported or corrupt audio; use WAV, AIFF, MP3 or M4A")?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .context("Audio has no default track")?;
    let id = track.id;
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let mut samples = Vec::new();
    let (mut rate, mut channels) = (0, 0);
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(e.into()),
        };
        if packet.track_id() != id {
            continue;
        }
        let audio = decoder
            .decode(&packet)
            .context("Audio packet could not be decoded")?;
        let spec = *audio.spec();
        ensure!(
            (1..=2).contains(&spec.channels.count()) && spec.rate > 0 && spec.rate <= 192000,
            "Only bounded mono/stereo audio is supported"
        );
        if rate == 0 {
            rate = spec.rate;
            channels = spec.channels.count();
        }
        ensure!(
            rate == spec.rate && channels == spec.channels.count(),
            "Audio format changes inside the source"
        );
        let mut buffer = SampleBuffer::<f32>::new(audio.capacity() as u64, spec);
        buffer.copy_interleaved_ref(audio);
        ensure!(
            samples.len() + buffer.samples().len() <= 57_600_000,
            "Decoded audio exceeds 220 MiB"
        );
        samples.extend_from_slice(buffer.samples());
        ensure!(
            samples.len() as f64 / channels as f64 / rate as f64 <= 600.,
            "Audio source exceeds ten minutes"
        );
    }
    ensure!(
        rate > 0 && !samples.is_empty() && samples.iter().all(|s| s.is_finite()),
        "Audio contains no usable finite samples"
    );
    Ok(Pcm {
        samples,
        rate,
        channels,
    })
}
pub fn normalize_wav(bytes: &[u8]) -> Result<Vec<u8>> {
    let pcm = decode(bytes)?;
    let mut out = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut out,
            hound::WavSpec {
                channels: pcm.channels as u16,
                sample_rate: pcm.rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )?;
        for sample in pcm.samples {
            writer.write_sample((sample.clamp(-1., 1.) * 32767.).round() as i16)?;
        }
        writer.finalize()?;
    }
    Ok(out.into_inner())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aiff_and_wav_decode_without_an_encoder() {
        // Big-endian AIFF, 2 mono samples at 44100 Hz.
        let mut aiff=b"FORM\0\0\0\x32AIFFCOMM\0\0\0\x12\0\x01\0\0\0\x02\0\x10\x40\x0e\xac\x44\0\0\0\0\0\0SSND\0\0\0\x0c\0\0\0\0\0\0\0\0".to_vec();
        aiff.extend_from_slice(&[0x40, 0, 0xc0, 0]);
        let pcm = decode(&aiff).unwrap();
        assert_eq!(pcm.rate, 44100);
        assert_eq!(pcm.channels, 1);
        assert!((pcm.samples[0] - 0.5).abs() < 0.0001);
        let wav = normalize_wav(&aiff).unwrap();
        assert!(wav.starts_with(b"RIFF"));
        assert_eq!(decode(&wav).unwrap().samples.len(), 2);
        assert!(decode(b"bad").is_err());
    }
}
