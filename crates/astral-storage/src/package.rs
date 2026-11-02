//! Portable project packages with a bounded manifest and verified media hashes.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};
const TOTAL: u64 = 2 * 1024 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u32,
    project: Id,
    revision: u64,
    files: Vec<Entry>,
}
#[derive(Serialize, Deserialize)]
struct Entry {
    path: String,
    bytes: u64,
    sha256: String,
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
impl Workspace {
    /// The archive is an immutable snapshot. Cancellation leaves no destination.
    pub fn package_project(
        &self,
        p: &Project,
        path: &str,
        check: impl Fn() -> Result<()>,
    ) -> Result<()> {
        ensure!(
            Path::new(path)
                .extension()
                .is_some_and(|e| e == "astralpack"),
            "Use .astralpack"
        );
        let target = self.path(path)?;
        ensure!(!target.exists(), "Package destination already exists");
        let parent = target.parent().context("Package folder missing")?;
        std::fs::create_dir_all(parent)?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        let mut manifest = Manifest {
            version: 1,
            project: p.id,
            revision: p.revision,
            files: vec![],
        };
        let mut seen = BTreeSet::new();
        let mut total = 0u64;
        {
            let mut zip = ZipWriter::new(temp.as_file_mut());
            let options = SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .unix_permissions(0o644);
            let bytes = encode(p)?;
            zip.start_file("project.astral", options)?;
            zip.write_all(&bytes)?;
            manifest.files.push(Entry {
                path: "project.astral".into(),
                bytes: bytes.len() as u64,
                sha256: digest(&bytes),
            });
            for a in &p.assets {
                check()?;
                if !seen.insert(a.path.to_lowercase()) {
                    continue;
                }
                ensure!(seen.len() <= 4096, "Too many packaged assets");
                let input = self.path(&a.path)?;
                ensure!(input != target, "Package cannot include itself");
                let bytes = self
                    .read(&a.path)
                    .with_context(|| format!("Missing or oversized media: {}", a.path))?;
                total += bytes.len() as u64;
                ensure!(total <= TOTAL, "Package media exceed 2 GiB");
                let name = format!("media/{}", a.path.replace('\\', "/"));
                zip.start_file(&name, options)?;
                zip.write_all(&bytes)?;
                manifest.files.push(Entry {
                    path: name,
                    bytes: bytes.len() as u64,
                    sha256: digest(&bytes),
                });
            }
            check()?;
            zip.start_file("manifest.json", options)?;
            zip.write_all(&serde_json::to_vec(&manifest)?)?;
            zip.finish()?;
        }
        check()?;
        temp.as_file().sync_all()?;
        temp.persist_noclobber(target).map_err(|e| e.error)?;
        Ok(())
    }
    /// Extraction always creates a new folder; no existing project or media is overwritten.
    pub fn unpack_project(
        &self,
        path: &str,
        destination: &str,
        check: impl Fn() -> Result<()>,
    ) -> Result<String> {
        let source = self.path(path)?;
        ensure!(source.metadata()?.len() <= TOTAL, "Package exceeds 2 GiB");
        let target = self.path(destination)?;
        ensure!(!target.exists(), "Unpack destination must be new");
        let parent = target.parent().context("Destination parent missing")?;
        std::fs::create_dir_all(parent)?;
        let temp = tempfile::tempdir_in(parent)?;
        let output = Workspace::new(temp.path())?;
        let mut zip = ZipArchive::new(std::fs::File::open(source)?)?;
        ensure!(zip.len() <= 4098, "Too many archive entries");
        let manifest_bytes = {
            let mut f = zip.by_name("manifest.json")?;
            ensure!(f.size() <= 2 * 1024 * 1024, "Manifest too large");
            let mut b = vec![];
            f.by_ref().take(2 * 1024 * 1024 + 1).read_to_end(&mut b)?;
            b
        };
        let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
        ensure!(
            manifest.version == 1 && manifest.files.len() + 1 == zip.len(),
            "Invalid package manifest/version"
        );
        let expected = manifest
            .files
            .iter()
            .map(|f| f.path.as_str())
            .chain(std::iter::once("manifest.json"))
            .collect::<BTreeSet<_>>();
        let mut archive_names = BTreeSet::new();
        for index in 0..zip.len() {
            let f = zip.by_index(index)?;
            ensure!(
                expected.contains(f.name()) && archive_names.insert(f.name().to_owned()),
                "Unexpected or duplicate archive entry"
            );
        }
        let mut seen = BTreeSet::new();
        let mut total = 0u64;
        for entry in &manifest.files {
            check()?;
            ensure!(
                seen.insert(entry.path.to_lowercase()),
                "Duplicate package entry"
            );
            let relative = if entry.path == "project.astral" {
                "project.astral"
            } else {
                entry
                    .path
                    .strip_prefix("media/")
                    .context("Unexpected package path")?
            };
            ensure!(
                relative != "manifest.json"
                    && (entry.path == "project.astral" || relative != "project.astral"),
                "Reserved package path"
            );
            ensure!(
                !relative.contains('\\')
                    && Path::new(relative)
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy())
                        .collect::<Vec<_>>()
                        .join("/")
                        == relative,
                "Noncanonical package path"
            );
            output.path(relative)?;
            let mut file = zip.by_name(&entry.path)?;
            ensure!(
                !file.is_dir() && file.unix_mode().is_none_or(|m| m & 0o170000 != 0o120000),
                "Links/directories are not package media"
            );
            ensure!(
                file.size() == entry.bytes && entry.bytes <= MAX_PROJECT_BYTES as u64,
                "Invalid packaged file size"
            );
            total += entry.bytes;
            ensure!(total <= TOTAL, "Expanded package exceeds 2 GiB");
            let mut bytes = vec![];
            file.by_ref()
                .take(entry.bytes + 1)
                .read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() as u64 == entry.bytes && digest(&bytes) == entry.sha256,
                "Package checksum mismatch: {}",
                entry.path
            );
            output.atomic_write(relative, &bytes)?;
        }
        let p = output.load("project.astral")?;
        ensure!(
            p.id == manifest.project && p.revision == manifest.revision,
            "Project identity does not match manifest"
        );
        for a in &p.assets {
            ensure!(
                seen.contains(&format!("media/{}", a.path.replace('\\', "/")).to_lowercase()),
                "Missing packaged asset"
            );
            output.path(&a.path)?;
        }
        check()?;
        std::fs::rename(temp.path(), &target)?;
        Ok(format!(
            "{}/project.astral",
            destination.trim_end_matches('/')
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn package_reopens_with_media_and_never_overwrites() {
        let d = tempfile::tempdir().unwrap();
        let w = Workspace::new(d.path()).unwrap();
        let mut p = Project::new("Portable", 320, 240, 24, 48);
        p.assets.push(Asset {
            id: Id::new_v4(),
            name: "Sound".into(),
            path: "assets/sound.wav".into(),
            kind: "audio".into(),
            tags: vec![],
        });
        w.atomic_write("assets/sound.wav", b"media").unwrap();
        w.package_project(&p, "film.astralpack", || Ok(())).unwrap();
        let path = w
            .unpack_project("film.astralpack", "unpacked", || Ok(()))
            .unwrap();
        let restored = Workspace::new(d.path().join("unpacked")).unwrap();
        assert_eq!(restored.load("project.astral").unwrap().id, p.id);
        assert_eq!(restored.read("assets/sound.wav").unwrap(), b"media");
        assert!(w.path(path).unwrap().is_file());
        assert!(
            w.unpack_project("film.astralpack", "unpacked", || Ok(()))
                .is_err()
        );
    }
    #[test]
    fn hostile_paths_and_bad_checksums_never_publish_a_folder() {
        let d = tempfile::tempdir().unwrap();
        let w = Workspace::new(d.path()).unwrap();
        for (index, name, hash) in [
            (0, "media/../escape", "valid"),
            (1, "media/a//b", "valid"),
            (2, "media/a", "bad"),
        ] {
            let mut bytes = std::io::Cursor::new(vec![]);
            let mut z = ZipWriter::new(&mut bytes);
            let opts = SimpleFileOptions::default();
            let payload = b"payload";
            let manifest = Manifest {
                version: 1,
                project: Id::new_v4(),
                revision: 0,
                files: vec![Entry {
                    path: name.into(),
                    bytes: payload.len() as u64,
                    sha256: if hash == "valid" {
                        digest(payload)
                    } else {
                        "0".repeat(64)
                    },
                }],
            };
            z.start_file(name, opts).unwrap();
            z.write_all(payload).unwrap();
            z.start_file("manifest.json", opts).unwrap();
            z.write_all(&serde_json::to_vec(&manifest).unwrap())
                .unwrap();
            z.finish().unwrap();
            let path = format!("hostile-{index}.astralpack");
            let destination = format!("output-{index}");
            w.atomic_write(&path, &bytes.into_inner()).unwrap();
            assert!(w.unpack_project(&path, &destination, || Ok(())).is_err());
            assert!(!w.path(&destination).unwrap().exists());
        }
    }
    #[test]
    fn cancellation_does_not_publish_partial_package() {
        let d = tempfile::tempdir().unwrap();
        let w = Workspace::new(d.path()).unwrap();
        let p = Project::new("Cancel", 32, 32, 24, 24);
        assert!(
            w.package_project(&p, "cancel.astralpack", || anyhow::bail!("Cancelled"))
                .is_err()
        );
        assert!(!w.path("cancel.astralpack").unwrap().exists());
    }
}
