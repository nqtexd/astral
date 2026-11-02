//! Streaming AVI 1.0 / Motion JPEG + PCM. No executable or external codec needed.
use anyhow::{Result, ensure};
use std::io::{Seek, SeekFrom, Write};
fn chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut v = tag.to_vec();
    v.extend_from_slice(&(data.len() as u32).to_le_bytes());
    v.extend_from_slice(data);
    if data.len() % 2 == 1 {
        v.push(0);
    }
    v
}
fn list(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut v = tag.to_vec();
    v.extend_from_slice(data);
    chunk(b"LIST", &v)
}
fn ints(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}
pub struct Avi<W> {
    writer: W,
    movi: u64,
    index: Vec<u8>,
}
impl<W: Write + Seek> Avi<W> {
    pub fn new(
        mut writer: W,
        width: u32,
        height: u32,
        fps: u32,
        frames: u32,
        audio: bool,
    ) -> Result<Self> {
        writer.write_all(b"RIFF\0\0\0\0AVI ")?;
        let mut hdrl = chunk(
            b"avih",
            &ints(&[
                1_000_000 / fps,
                0,
                0,
                0x10,
                frames,
                0,
                if audio { 2 } else { 1 },
                width * height * 3,
                width,
                height,
                0,
                0,
                0,
                0,
            ]),
        );
        let mut strh = b"vidsMJPG".to_vec();
        strh.extend(ints(&[
            0,
            0,
            0,
            1,
            fps,
            0,
            frames,
            width * height * 3,
            u32::MAX,
            0,
        ]));
        strh.extend(
            [0i16, 0, width as i16, height as i16]
                .into_iter()
                .flat_map(i16::to_le_bytes),
        );
        let mut strl = chunk(b"strh", &strh);
        let mut bitmap = ints(&[40, width, height]);
        bitmap.extend_from_slice(&[1, 0, 24, 0]);
        bitmap.extend_from_slice(b"MJPG");
        bitmap.extend(ints(&[width * height * 3, 0, 0, 0, 0]));
        strl.extend(chunk(b"strf", &bitmap));
        hdrl.extend(list(b"strl", &strl));
        if audio {
            let count = (frames as u64 * 48000 / fps as u64) as u32;
            let mut h = b"auds\0\0\0\0".to_vec();
            h.extend(ints(&[0, 0, 0, 4, 192000, 0, count, 192000, u32::MAX, 4]));
            h.extend_from_slice(&[0; 8]);
            let mut l = chunk(b"strh", &h);
            let mut fmt = vec![1, 0, 2, 0];
            fmt.extend(ints(&[48000, 192000]));
            fmt.extend_from_slice(&[4, 0, 16, 0]);
            l.extend(chunk(b"strf", &fmt));
            hdrl.extend(list(b"strl", &l));
        }
        writer.write_all(&list(b"hdrl", &hdrl))?;
        let movi = writer.stream_position()?;
        writer.write_all(b"LIST\0\0\0\0movi")?;
        Ok(Self {
            writer,
            movi,
            index: vec![],
        })
    }
    pub fn frame(&mut self, jpeg: &[u8], pcm: Option<&[u8]>) -> Result<()> {
        self.write_chunk(b"00dc", jpeg, 0x10)?;
        if let Some(pcm) = pcm {
            self.write_chunk(b"01wb", pcm, 0)?;
        }
        Ok(())
    }
    fn write_chunk(&mut self, tag: &[u8; 4], data: &[u8], flags: u32) -> Result<()> {
        let pos = self.writer.stream_position()?;
        ensure!(
            pos + data.len() as u64 + 16 < 2_000_000_000,
            "Native AVI reaches its 2 GB safety limit; choose MP4 or an image sequence for larger work"
        );
        self.index.extend_from_slice(tag);
        self.index.extend(ints(&[
            flags,
            (pos - self.movi - 8) as u32,
            data.len() as u32,
        ]));
        self.writer.write_all(&chunk(tag, data))?;
        Ok(())
    }
    pub fn finish(mut self) -> Result<W> {
        let end = self.writer.stream_position()?;
        self.writer.seek(SeekFrom::Start(self.movi + 4))?;
        self.writer
            .write_all(&((end - self.movi - 8) as u32).to_le_bytes())?;
        self.writer.seek(SeekFrom::Start(end))?;
        self.writer.write_all(&chunk(b"idx1", &self.index))?;
        let end = self.writer.stream_position()?;
        ensure!(end < 2_000_000_000, "Native AVI exceeds 2 GB");
        self.writer.seek(SeekFrom::Start(4))?;
        self.writer.write_all(&((end - 8) as u32).to_le_bytes())?;
        self.writer.seek(SeekFrom::Start(end))?;
        self.writer.flush()?;
        Ok(self.writer)
    }
}
