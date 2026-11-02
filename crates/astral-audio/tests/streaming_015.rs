use astral_audio::*;
use astral_core::*;
use astral_storage::Workspace;
use std::io::Cursor;
#[test]
fn trimmed_stream_retains_speed_and_original_fade_phase() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::new(dir.path()).unwrap();
    let mut input = Cursor::new(vec![]);
    {
        let mut w = hound::WavWriter::new(
            &mut input,
            hound::WavSpec {
                channels: 1,
                sample_rate: 48000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for i in 0..144000 {
            w.write_sample(((i % 997) as f32 / 997. * 20000.) as i16)
                .unwrap();
        }
        w.finalize().unwrap();
    }
    workspace
        .atomic_write("source.wav", &input.into_inner())
        .unwrap();
    let mut p = Project::new("Audio", 64, 64, 24, 24);
    p.schema = 9;
    let asset = Id::new_v4();
    p.assets.push(Asset {
        id: asset,
        name: "Source".into(),
        path: "source.wav".into(),
        kind: "audio".into(),
        tags: vec![],
    });
    p.audio.push(AudioClip {
        id: Id::new_v4(),
        name: "Sped up".into(),
        asset,
        start: 0,
        offset_seconds: 0.1,
        duration_seconds: 1.,
        gain: 0.8,
        fade_in: 0.5,
        fade_out: 0.3,
        looping: false,
        speed: 2.,
    });
    let full = mix(&p, &workspace).unwrap();
    let full: Vec<_> = hound::WavReader::new(Cursor::new(full))
        .unwrap()
        .samples::<i16>()
        .map(Result::unwrap)
        .collect();
    let mut range = Cursor::new(vec![]);
    mix_range_to(&p, &workspace, 0.25, 0.75, &mut range, || Ok(())).unwrap();
    let range: Vec<_> = hound::WavReader::new(Cursor::new(range.into_inner()))
        .unwrap()
        .samples::<i16>()
        .map(Result::unwrap)
        .collect();
    assert_eq!(range.len(), 48000);
    assert!(
        range
            .iter()
            .zip(&full[24000..72000])
            .all(|(a, b)| (*a as i32 - *b as i32).abs() <= 1)
    );
}
#[test]
fn streaming_mix_observes_cancellation_between_bounded_blocks() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::new(dir.path()).unwrap();
    let p = Project::new("Long", 64, 64, 24, 24 * 3600);
    let mut output = Cursor::new(vec![]);
    let mut calls = 0;
    let result = mix_range_to(&p, &workspace, 0., 3600., &mut output, || {
        calls += 1;
        if calls >= 3 {
            anyhow::bail!("Cancelled");
        }
        Ok(())
    });
    assert!(result.is_err());
    assert!(output.get_ref().len() < 1_000_000);
}
