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
        let bytes = encode(project)?;
        let path = path.as_ref();
        let target = self.path(path)?;
        // Keep the last five distinct saved versions before replacing the destination.
        // Recovery saves are already a rolling copy and must not multiply backups.
        if !path.components().any(|c| c.as_os_str() == "recovery") && target.is_file() {
            let previous = self.read(path)?;
            if previous != bytes {
                let digest = Sha256::digest(path.to_string_lossy().as_bytes());
                let key = digest
                    .iter()
                    .take(12)
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>();
                let folder = format!(".astral-backups/{key}");
                let name = format!(
                    "{folder}/{}-{}.astral",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_nanos(),
                    Id::new_v4()
                );
                self.atomic_write(&name, &previous)?;
                let directory = self.path(&folder)?;
                let mut entries = std::fs::read_dir(&directory)?
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.file_type().is_ok_and(|t| t.is_file())
                            && e.path().extension().is_some_and(|s| s == "astral")
                    })
                    .collect::<Vec<_>>();
                entries.sort_by_key(|e| e.file_name());
                let excess = entries.len().saturating_sub(5);
                for entry in entries.into_iter().take(excess) {
                    std::fs::remove_file(entry.path())?;
                }
            }
        }
        self.atomic_write(path, &bytes)
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
    /// Never replace a newer crash snapshot with an older saved document.
    pub fn save_recovery(&self, project: &Project) -> Result<()> {
        if let Ok(previous) = self.recover(project.id)
            && previous.revision > project.revision
        {
            return Ok(());
        }
        self.save(Self::recovery_path(project.id), project)
    }
    /// Recover only ordinary project files. Opening a backup is an explicit choice.
    /// The second value is the revision actually saved at the requested path.
    pub fn load_recoverable(&self, path: impl AsRef<Path>) -> Result<(Project, u64)> {
        let path = path.as_ref();
        let saved = self.load(path)?;
        let revision = saved.revision;
        if !path
            .components()
            .any(|c| matches!(c.as_os_str().to_str(), Some("recovery" | ".astral-backups")))
        {
            let recovery = Self::recovery_path(saved.id);
            if let Ok(candidate) = self.recover(saved.id)
                && candidate.id == saved.id
                && candidate.revision > revision
                && self.path(&recovery)?.metadata()?.modified()?
                    >= self.path(path)?.metadata()?.modified()?
            {
                return Ok((candidate, revision));
            }
        }
        Ok((saved, revision))
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
    fn newer_recovery_survives_stale_open_and_reopens_as_unsaved() {
        let dir = tempfile::tempdir().unwrap();
        let w = Workspace::new(dir.path()).unwrap();
        let mut p = fight_demo().unwrap();
        p.revision = 3;
        w.save("film.astral", &p).unwrap();
        let saved = p.clone();
        p.revision = 8;
        p.name = "Recovered work".into();
        w.save_recovery(&p).unwrap();
        w.save_recovery(&saved).unwrap();
        assert_eq!(w.recover(p.id).unwrap().revision, 8);
        let (loaded, saved_revision) = w.load_recoverable("film.astral").unwrap();
        assert_eq!(loaded.name, "Recovered work");
        assert_eq!(saved_revision, 3);
        assert_eq!(w.load("film.astral").unwrap().revision, 3);
        let (_, explicit_revision) = w.load_recoverable(Workspace::recovery_path(p.id)).unwrap();
        assert_eq!(explicit_revision, 8);
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

/// Prefer the packaged parser runtime. No user project can select executable paths.
pub fn node_executable(application_root: &std::path::Path) -> std::path::PathBuf {
    let name = if cfg!(windows) { "node.exe" } else { "node" };
    let bundled = application_root.join("resources/node").join(name);
    if bundled.is_file() {
        return bundled;
    }
    let staged = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../build/astral-node")
        .join(name);
    if staged.is_file() {
        return staged;
    }
    "node".into()
}

mod package;
