//! Bounded, checksummed project containers and confined atomic filesystem I/O.
use anyhow::{Context, Result, ensure};
use astral_core::*;
use flate2::{Compression, bufread::ZlibDecoder, write::ZlibEncoder};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

pub const MAX_PROJECT_BYTES: usize = 256 * 1024 * 1024;
const HEADER: usize = 60;

pub fn encode(project: &Project) -> Result<Vec<u8>> {
    project.validate()?;
    let raw = serde_json::to_vec(project)?;
    ensure!(
        raw.len() <= MAX_PROJECT_BYTES,
        "Project exceeds 256 MiB limit"
    );
    let mut z = ZlibEncoder::new(Vec::new(), Compression::fast());
    z.write_all(&raw)?;
    let packed = z.finish()?;
    let mut out = Vec::with_capacity(HEADER + packed.len());
    out.extend_from_slice(b"ASTL");
    out.extend_from_slice(&[1, 1, 0, 0]);
    out.extend_from_slice(&(raw.len() as u64).to_le_bytes());
    out.extend_from_slice(&(packed.len() as u64).to_le_bytes());
    out.extend_from_slice(&Sha256::digest(&raw));
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&packed);
    ensure!(
        out.len() <= MAX_PROJECT_BYTES,
        "Compressed project exceeds limit"
    );
    Ok(out)
}
pub fn decode(bytes: &[u8]) -> Result<Project> {
    ensure!(
        bytes.len() >= HEADER && bytes.len() <= MAX_PROJECT_BYTES,
        "Invalid Astral file size"
    );
    ensure!(
        &bytes[..4] == b"ASTL" && bytes[4..8] == [1, 1, 0, 0],
        "Unsupported Astral container"
    );
    let raw_len = u64::from_le_bytes(bytes[8..16].try_into()?);
    let packed_len = u64::from_le_bytes(bytes[16..24].try_into()?);
    ensure!(
        raw_len <= MAX_PROJECT_BYTES as u64
            && packed_len == (bytes.len() - HEADER) as u64
            && bytes[56..60] == [0; 4],
        "Invalid section or payload lengths"
    );
    let mut z = ZlibDecoder::new(&bytes[HEADER..]);
    let mut raw = Vec::new();
    z.by_ref().take(raw_len + 1).read_to_end(&mut raw)?;
    ensure!(
        raw.len() as u64 == raw_len && z.get_ref().is_empty(),
        "Truncated or excessive project data"
    );
    ensure!(
        Sha256::digest(&raw)[..] == bytes[24..56],
        "Project checksum mismatch"
    );
    let project: Project = serde_json::from_slice(&raw)?;
    project.validate()?;
    Ok(project)
}

#[derive(Clone)]
pub struct Workspace {
    root: PathBuf,
}
impl Workspace {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        std::fs::create_dir_all(root.as_ref())?;
        Ok(Self {
            root: root.as_ref().canonicalize()?,
        })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn path(&self, relative: impl AsRef<Path>) -> Result<PathBuf> {
        let relative = relative.as_ref();
        ensure!(
            !relative.as_os_str().is_empty()
                && relative
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
                && !relative.to_string_lossy().contains(':'),
            "Use a relative workspace path without traversal"
        );
        let candidate = self.root.join(relative);
        let mut ancestor = candidate.as_path();
        while !ancestor.exists() {
            ancestor = ancestor.parent().context("Invalid path")?;
        }
        ensure!(
            ancestor.canonicalize()?.starts_with(&self.root),
            "Path escapes workspace through a link"
        );
        Ok(candidate)
    }
    pub fn read(&self, path: impl AsRef<Path>) -> Result<Vec<u8>> {
        let path = self.path(path)?;
        ensure!(
            std::fs::metadata(&path)?.len() <= MAX_PROJECT_BYTES as u64,
            "File exceeds 256 MiB limit"
        );
        Ok(std::fs::read(path)?)
    }
    pub fn load(&self, path: impl AsRef<Path>) -> Result<Project> {
        decode(&self.read(path)?)
    }
    pub fn save(&self, path: impl AsRef<Path>, project: &Project) -> Result<()> {
        ensure!(
            path.as_ref().extension().is_some_and(|e| e == "astral"),
            "Save projects with .astral extension"
        );
        self.atomic_write(path, &encode(project)?)
    }
    pub fn atomic_write(&self, path: impl AsRef<Path>, bytes: &[u8]) -> Result<()> {
        let target = self.path(path)?;
        let parent = target.parent().context("Missing parent")?;
        std::fs::create_dir_all(parent)?;
        ensure!(
            parent.canonicalize()?.starts_with(&self.root),
            "Destination escapes workspace"
        );
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(bytes)?;
        temp.as_file().sync_all()?;
        temp.persist(&target).map_err(|e| e.error)?;
        Ok(())
    }
    /// Viewers on Windows may hold an old preview without delete sharing.
    /// Preserve that file and return a fresh path instead of failing the render.
    pub fn write_preview(&self, path: &str, bytes: &[u8]) -> Result<(String, Vec<String>)> {
        match self.atomic_write(path, bytes) {
            Ok(()) => Ok((path.to_owned(), vec![])),
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::PermissionDenied) =>
            {
                let requested = self.path(path)?;
                ensure!(
                    requested.is_file(),
                    "Preview destination is not a file: {error}"
                );
                let name = requested
                    .file_stem()
                    .context("Preview filename missing")?
                    .to_string_lossy();
                let extension = requested
                    .extension()
                    .context("Preview extension missing")?
                    .to_string_lossy();
                let fresh =
                    requested.with_file_name(format!("{name}-{}.{}", Id::new_v4(), extension));
                let relative = fresh
                    .strip_prefix(&self.root)?
                    .to_string_lossy()
                    .replace('\\', "/");
                self.atomic_write(&relative, bytes)?;
                Ok((
                    relative,
                    vec![format!(
                        "Could not replace {path}; it may be open in another application or read-only. The existing file was preserved. Use the returned preview path."
                    )],
                ))
            }
            Err(error) => Err(error),
        }
    }
    pub fn recovery_path(id: Id) -> String {
        format!("recovery/{id}.astral")
    }
    pub fn recover(&self, id: Id) -> Result<Project> {
        self.load(Self::recovery_path(id))
    }
    pub fn save_recovery(&self, project: &Project) -> Result<()> {
        self.save(Self::recovery_path(project.id), project)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn container_rejects_corruption_and_limits() {
        let p = fight_demo().unwrap();
        let bytes = encode(&p).unwrap();
        assert_eq!(decode(&bytes).unwrap().id, p.id);
        let mut bad = bytes.clone();
        bad[24] ^= 1;
        assert!(decode(&bad).is_err());
        let mut bad = bytes.clone();
        bad[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(decode(&bad).is_err());
        let mut bad = bytes;
        bad.push(0);
        assert!(decode(&bad).is_err());
    }
    #[test]
    fn atomic_save_reopen_recovery_and_confinement() {
        let dir = tempfile::tempdir().unwrap();
        let w = Workspace::new(dir.path()).unwrap();
        let mut p = fight_demo().unwrap();
        w.save("test.astral", &p).unwrap();
        p.name = "Replacement".into();
        w.save("test.astral", &p).unwrap();
        assert_eq!(w.load("test.astral").unwrap().name, "Replacement");
        w.save_recovery(&p).unwrap();
        assert_eq!(w.recover(p.id).unwrap().id, p.id);
        assert!(w.path("../outside").is_err());
        assert!(w.path("C:/outside").is_err());
        assert!(w.save("original.piv", &p).is_err());
    }
}

#[cfg(all(test, windows))]
mod preview_lock_tests {
    use super::*;
    use std::os::windows::fs::OpenOptionsExt;
    #[test]
    fn locked_preview_is_versioned_without_destroying_the_viewed_file() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Workspace::new(dir.path()).unwrap();
        workspace.atomic_write("preview.png", b"old").unwrap();
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(dir.path().join("preview.png"))
            .unwrap();
        let (path, warnings) = workspace.write_preview("preview.png", b"new").unwrap();
        assert_ne!(path, "preview.png");
        assert_eq!(warnings.len(), 1);
        assert_eq!(workspace.read(&path).unwrap(), b"new");
        drop(lock);
        assert_eq!(workspace.read("preview.png").unwrap(), b"old");
        let (path, warnings) = workspace
            .write_preview("preview.png", b"replacement")
            .unwrap();
        assert_eq!(path, "preview.png");
        assert!(warnings.is_empty());
        assert_eq!(workspace.read(path).unwrap(), b"replacement");
    }
}
