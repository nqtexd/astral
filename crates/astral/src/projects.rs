//! Trusted desktop file selection configures a confined root for each document.
//! Agent requests cannot choose arbitrary filesystem roots.
use crate::service::Service;
use anyhow::{Context, Result, ensure};
use astral_core::*;
use astral_storage::{MAX_PROJECT_BYTES, Workspace};
use std::path::{Path, PathBuf};

impl Service {
    pub fn project_workspace(&self, id: Id) -> Workspace {
        self.store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .locations
            .get(&id)
            .cloned()
            .unwrap_or_else(|| self.workspace.clone())
    }
    pub fn project_file(&self, id: Id) -> Option<PathBuf> {
        self.store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .files
            .get(&id)
            .cloned()
    }
    /// Only the native file picker/library calls this; never exposed as an MCP path operation.
    pub fn open_local_project(&self, file: &Path) -> Result<Id> {
        let file = file.canonicalize()?;
        ensure!(
            std::fs::metadata(&file)?.len() <= MAX_PROJECT_BYTES as u64,
            "Project exceeds 256 MiB"
        );
        let p = astral_storage::decode(&std::fs::read(&file)?)?;
        {
            let store = self.store.lock().unwrap();
            if store.sessions.contains_key(&p.id) {
                ensure!(
                    store.files.get(&p.id).is_some_and(|path| path == &file),
                    "This project is already open from another location"
                );
                return Ok(p.id);
            }
        }
        let parent = Workspace::new(file.parent().context("Project folder missing")?)?;
        let complete = |w: &Workspace| {
            p.assets
                .iter()
                .all(|a| w.path(&a.path).is_ok_and(|path| path.is_file()))
        };
        let workspace = if !complete(&parent) && complete(&self.workspace) {
            self.workspace.clone()
        } else {
            parent
        };
        let revision = p.revision;
        let id = self.insert(p)?;
        let mut store = self.store.lock().unwrap();
        store.locations.insert(id, workspace);
        store.files.insert(id, file);
        store.saved_revisions.insert(id, revision);
        Ok(id)
    }
    /// Save As copies linked assets without overwriting unrelated existing media.
    /// Session identity/history and relative asset paths are preserved.
    pub fn save_local_project_as(&self, id: Id, file: &Path) -> Result<PathBuf> {
        ensure!(
            file.extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("astral")),
            "Choose an .astral file"
        );
        let parent = file.parent().context("Project folder missing")?;
        let filename = file.file_name().context("Project filename missing")?;
        let destination = Workspace::new(parent)?;
        let source = self.project_workspace(id);
        let p = self.snapshot(id)?;
        let activity = self.project_activity(id).unwrap_or_default();
        let target = destination.path(Path::new(filename))?;
        for asset in &p.assets {
            ensure!(
                destination.path(&asset.path)? != target,
                "Project filename conflicts with linked media"
            );
        }
        if source.root() != destination.root() {
            // Validate all assets and conflicts before writing any output.
            for asset in &p.assets {
                let input = source.path(&asset.path)?;
                ensure!(input.is_file(), "Missing linked media: {}", asset.path);
                ensure!(
                    std::fs::metadata(&input)?.len() <= MAX_PROJECT_BYTES as u64,
                    "Asset exceeds 256 MiB"
                );
                let output = destination.path(&asset.path)?;
                if output.exists() {
                    ensure!(
                        source.read(&asset.path)? == destination.read(&asset.path)?,
                        "Save As would overwrite unrelated media: {}",
                        asset.path
                    );
                }
            }
            for asset in &p.assets {
                if !destination.path(&asset.path)?.exists() {
                    destination.atomic_write(&asset.path, &source.read(&asset.path)?)?;
                }
            }
        }
        destination.save(Path::new(filename), &p)?;
        if !activity.is_empty() {
            let _ = destination.atomic_write(
                format!(".astral-history/{id}.json"),
                &serde_json::to_vec_pretty(&activity)?,
            );
        }
        let absolute = destination.path(Path::new(filename))?.canonicalize()?;
        let mut store = self.store.lock().unwrap();
        store.locations.insert(id, destination);
        store.files.insert(id, absolute.clone());
        store.saved_revisions.insert(id, p.revision);
        Ok(absolute)
    }
    pub fn close_local_project(&self, id: Id) -> Result<()> {
        let mut store = self.store.lock().unwrap();
        ensure!(store.sessions.remove(&id).is_some(), "Project is not open");
        store.locations.remove(&id);
        store.files.remove(&id);
        store.saved_revisions.remove(&id);
        Ok(())
    }
    pub fn saved_project_files(&self) -> Result<serde_json::Value> {
        let mut found = vec![];
        let mut queue =
            std::collections::VecDeque::from([(self.workspace.root().to_path_buf(), 0)]);
        while let Some((folder, depth)) = queue.pop_front() {
            for entry in std::fs::read_dir(folder)?.take(4096) {
                let entry = entry?;
                let kind = entry.file_type()?;
                if kind.is_symlink() {
                    continue;
                }
                if kind.is_dir() && depth < 3 {
                    let name = entry.file_name();
                    if ![
                        "assets",
                        "recovery",
                        ".settings",
                        ".astral-trash",
                        ".astral-history",
                    ]
                    .iter()
                    .any(|v| name.eq_ignore_ascii_case(v))
                    {
                        queue.push_back((entry.path(), depth + 1));
                    }
                } else if kind.is_file()
                    && entry
                        .path()
                        .extension()
                        .is_some_and(|v| v.eq_ignore_ascii_case("astral"))
                {
                    let meta = entry.metadata()?;
                    found.push(serde_json::json!({"path":entry.path().strip_prefix(self.workspace.root())?,"bytes":meta.len(),"modified_unix":meta.modified().ok().and_then(|t|t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d|d.as_secs())}));
                    if found.len() >= 512 {
                        return Ok(serde_json::json!({"files":found,"truncated":true}));
                    }
                }
            }
        }
        found.sort_by_key(|v| v["path"].as_str().unwrap_or("").to_string());
        Ok(serde_json::json!({"files":found,"truncated":false}))
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ActivityEntry {
    pub revision: u64,
    pub timestamp: u64,
    pub action: String,
    pub changes: Vec<String>,
}
fn changed_sections(before: &Project, after: &Project) -> Vec<String> {
    let summarize = |p: &Project| {
        let mut light = p.clone();
        for b in &mut light.bitmaps {
            b.png = "".into();
        }
        serde_json::to_value(light).unwrap_or_default()
    };
    let a = summarize(before);
    let b = summarize(after);
    [
        "name",
        "width",
        "height",
        "fps",
        "duration",
        "background",
        "characters",
        "actions",
        "effects",
        "audio",
        "camera",
        "drawings",
        "layers",
        "cels",
        "assets",
        "poses",
        "dialogues",
        "graphics",
        "bitmaps",
        "rasters",
        "mouths",
        "metadata",
    ]
    .into_iter()
    .filter(|key| a[*key] != b[*key])
    .map(|key| key.to_string())
    .collect()
}
impl Service {
    fn activity_workspace(&self, id: Id) -> Result<Workspace> {
        if let Some(file) = self.project_file(id) {
            Workspace::new(file.parent().context("Project folder missing")?)
        } else {
            Ok(self.project_workspace(id))
        }
    }
    pub fn project_activity(&self, id: Id) -> Result<Vec<ActivityEntry>> {
        let workspace = self.activity_workspace(id)?;
        let path = format!(".astral-history/{id}.json");
        if !workspace.path(&path)?.exists() {
            return Ok(vec![]);
        }
        let bytes = workspace.read(path)?;
        ensure!(bytes.len() <= 1024 * 1024, "Activity log exceeds budget");
        Ok(serde_json::from_slice(&bytes)?)
    }
    pub fn record_activity(&self, before: &Project, after: &Project, action: &str) -> Result<()> {
        let _guard = self
            .activity_lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Activity log unavailable"))?;
        let mut entries = self.project_activity(after.id)?;
        entries.push(ActivityEntry {
            revision: after.revision,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
            action: action.into(),
            changes: changed_sections(before, after),
        });
        entries.sort_by_key(|entry| entry.revision);
        if entries.len() > 200 {
            entries.drain(..entries.len() - 200);
        }
        self.activity_workspace(after.id)?.atomic_write(
            format!(".astral-history/{}.json", after.id),
            &serde_json::to_vec_pretty(&entries)?,
        )
    }
    pub fn rename_local_project(
        &self,
        id: Id,
        expected_revision: u64,
        name: String,
    ) -> Result<serde_json::Value> {
        let name = name.trim();
        ensure!(
            !name.is_empty() && name.chars().count() <= 120,
            "Use a project name of 1–120 characters"
        );
        let before = self.snapshot(id)?;
        let receipt = self.request(crate::service::Request::ApplyCommands {
            batch: Batch {
                project: id,
                expected_revision,
                commands: vec![Command::SetProject {
                    name: Some(name.into()),
                    duration: None,
                    background: None,
                }],
            },
        })?;
        // Renaming the title is undoable. The original filename and linked media remain stable.
        Ok(
            serde_json::json!({"receipt":receipt,"project":id,"revision":receipt["revision"],"name":name,"saved":false,"note":"Title renamed. Save to persist it; the filename and linked assets stay unchanged.","previous_name":before.name}),
        )
    }
    pub fn trash_local_project(&self, id: Id, expected_revision: u64) -> Result<PathBuf> {
        let mut store = self
            .store
            .lock()
            .map_err(|_| anyhow::anyhow!("Document service unavailable"))?;
        let session = store.sessions.get(&id).context("Project is not open")?;
        ensure!(
            session.project.revision == expected_revision,
            "Stale revision: inspect before moving a project to Trash"
        );
        let file = store
            .files
            .get(&id)
            .context("Save the project before moving it to Trash")?
            .clone();
        let workspace = Workspace::new(file.parent().context("Project folder missing")?)?;
        let filename = file.file_name().context("Project filename missing")?;
        let target = workspace.path(format!(
            ".astral-trash/{}--{}",
            Id::new_v4(),
            filename.to_string_lossy()
        ))?;
        std::fs::create_dir_all(target.parent().unwrap())?;
        // Save the current editable state first; a failed move never removes the original.
        workspace.save(Path::new(filename), &session.project)?;
        std::fs::rename(&file, &target)?;
        store.sessions.remove(&id);
        store.files.remove(&id);
        store.locations.remove(&id);
        store.saved_revisions.remove(&id);
        Ok(target)
    }
    pub fn restore_local_project(&self, trash: &Path) -> Result<Id> {
        let trash = trash.canonicalize()?;
        let folder = trash.parent().context("Trash folder missing")?;
        ensure!(
            folder.file_name().is_some_and(|n| n == ".astral-trash"),
            "Choose a project in Astral Trash"
        );
        let filename = trash
            .file_name()
            .context("Trash filename missing")?
            .to_string_lossy();
        let (token, original) = filename
            .split_once("--")
            .context("Invalid trash filename")?;
        Id::parse_str(token)?;
        ensure!(
            Path::new(original).components().count() == 1
                && Path::new(original)
                    .extension()
                    .is_some_and(|e| e == "astral"),
            "Invalid original project filename"
        );
        let workspace = Workspace::new(folder.parent().context("Original folder missing")?)?;
        let target = workspace.path(original)?;
        ensure!(
            !target.exists(),
            "A file already exists at the original location; it will not be overwritten"
        );
        let project = astral_storage::decode(&std::fs::read(&trash)?)?;
        ensure!(
            !self
                .store
                .lock()
                .unwrap()
                .sessions
                .contains_key(&project.id),
            "Close the open copy before restoring this project"
        );
        std::fs::rename(&trash, &target)?;
        self.open_local_project(&target)
    }
}
