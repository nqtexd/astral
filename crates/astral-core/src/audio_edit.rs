use crate::*;
use anyhow::{Result, ensure};
/// Sample-continuous source offsets. Refuse a cut inside a fade rather than altering its envelope.
pub fn split_audio(clip: &AudioClip, frame: u32, fps: u32) -> Result<(AudioClip, AudioClip)> {
    ensure!(
        fps > 0 && frame > clip.start,
        "Split must be after the clip start"
    );
    let seconds = (frame - clip.start) as f64 / fps as f64;
    ensure!(
        seconds >= 0.01 && clip.duration_seconds - seconds >= 0.01,
        "Split must be inside the clip"
    );
    ensure!(
        seconds >= clip.fade_in as f64 && clip.duration_seconds - seconds >= clip.fade_out as f64,
        "Move the split outside the fade ranges to preserve the audio envelope"
    );
    let mut left = clip.clone();
    left.duration_seconds = seconds;
    left.fade_out = 0.;
    let mut right = clip.clone();
    right.id = Id::new_v4();
    right.start = frame;
    right.offset_seconds += seconds * clip.speed as f64;
    right.duration_seconds -= seconds;
    right.fade_in = 0.;
    Ok((left, right))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_keeps_source_time_speed_and_outer_fades() {
        let c = AudioClip {
            id: Id::new_v4(),
            name: "Voice".into(),
            asset: Id::new_v4(),
            start: 24,
            offset_seconds: 3.,
            duration_seconds: 4.,
            gain: 0.8,
            speed: 2.,
            fade_in: 0.2,
            fade_out: 0.3,
            looping: true,
        };
        let (a, b) = split_audio(&c, 72, 24).unwrap();
        assert_eq!(a.duration_seconds + b.duration_seconds, 4.);
        assert_eq!(b.offset_seconds, 7.);
        assert_eq!(a.fade_in, c.fade_in);
        assert_eq!(b.fade_out, c.fade_out);
        assert!(split_audio(&c, 25, 24).is_err());
        assert!(split_audio(&c, 120, 24).is_err());
    }
}
