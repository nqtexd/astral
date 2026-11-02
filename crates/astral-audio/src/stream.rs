//! Mix in two-second blocks, preserving the original source timeline and fades.
use crate::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Seek, Write},
    sync::Arc,
};
pub fn mix_range_to<W: Write + Seek>(
    p: &Project,
    workspace: &Workspace,
    first: f64,
    last: f64,
    output: W,
    mut check: impl FnMut() -> Result<()>,
) -> Result<()> {
    ensure!(
        first.is_finite()
            && last.is_finite()
            && first >= 0.
            && last > first
            && last - first <= 6. * 3600.,
        "Mix a range of at most six hours"
    );
    let frames = ((last - first) * SAMPLE_RATE as f64).round() as usize;
    let mut writer = hound::WavWriter::new(
        output,
        hound::WavSpec {
            channels: 2,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )?;
    let mut sources: BTreeMap<Id, Arc<Pcm>> = BTreeMap::new();
    let block = SAMPLE_RATE as usize * 2;
    for offset in (0..frames).step_by(block) {
        check()?;
        let n = block.min(frames - offset);
        let begin = first + offset as f64 / SAMPLE_RATE as f64;
        let end = begin + n as f64 / SAMPLE_RATE as f64;
        let active: Vec<_> = p
            .audio
            .iter()
            .filter(|a| {
                let start = a.start as f64 / p.fps as f64;
                start < end && start + a.duration_seconds > begin
            })
            .collect();
        let ids: BTreeSet<_> = active.iter().map(|a| a.asset).collect();
        sources.retain(|id, _| ids.contains(id));
        for id in ids {
            if let std::collections::btree_map::Entry::Vacant(entry) = sources.entry(id) {
                let asset = p
                    .assets
                    .iter()
                    .find(|a| a.id == id)
                    .context("Audio asset missing")?;
                let pcm = decode(&workspace.read(&asset.path)?)?;
                entry.insert(Arc::new(pcm));
                ensure!(
                    sources.values().map(|s| s.samples.len() * 4).sum::<usize>()
                        <= 256 * 1024 * 1024,
                    "Simultaneously active decoded audio exceeds 256 MiB; consolidate tracks"
                );
            }
        }
        let mut mixed = vec![[0f32; 2]; n];
        for clip in active {
            let pcm = &sources[&clip.asset];
            let source_frames = pcm.samples.len() / pcm.channels;
            ensure!(source_frames > 0, "Audio source is empty");
            for (i, out) in mixed.iter_mut().enumerate() {
                let time = first + (offset + i) as f64 / SAMPLE_RATE as f64
                    - clip.start as f64 / p.fps as f64;
                if time < 0. || time >= clip.duration_seconds {
                    continue;
                }
                let pos = (clip.offset_seconds + time * clip.speed as f64) * pcm.rate as f64;
                let pos = if clip.looping {
                    pos.rem_euclid(source_frames as f64)
                } else {
                    if pos >= source_frames as f64 {
                        continue;
                    }
                    pos
                };
                let a = pos.floor() as usize;
                let b = if clip.looping {
                    (a + 1) % source_frames
                } else {
                    (a + 1).min(source_frames - 1)
                };
                let t = (pos - a as f64) as f32;
                let fade_in = if clip.fade_in > 0. {
                    (time as f32 / clip.fade_in).clamp(0., 1.)
                } else {
                    1.
                };
                let fade_out = if clip.fade_out > 0. {
                    ((clip.duration_seconds - time) as f32 / clip.fade_out).clamp(0., 1.)
                } else {
                    1.
                };
                for (ch, value) in out.iter_mut().enumerate() {
                    let c = ch.min(pcm.channels - 1);
                    let v = pcm.samples[a * pcm.channels + c];
                    let next = pcm.samples[b * pcm.channels + c];
                    *value += (v + (next - v) * t) * clip.gain * fade_in * fade_out;
                }
            }
        }
        for frame in mixed {
            for value in frame {
                writer.write_sample((value.clamp(-1., 1.) * i16::MAX as f32) as i16)?;
            }
        }
    }
    check()?;
    writer.finalize()?;
    Ok(())
}
