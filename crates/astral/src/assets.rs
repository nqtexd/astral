//! Confined media management shared by the desktop, CLI and MCP.
use crate::service::{Request, Service};
use anyhow::{Context, Result, ensure};
use astral_core::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
#[schemars(extend("type" = "object"))]
pub enum AssetRequest {
    Inspect {
        project: Id,
    },
    Relink {
        project: Id,
        expected_revision: u64,
        asset: Id,
        path: String,
    },
    Package {
        project: Id,
        expected_revision: u64,
        path: String,
    },
    Unpack {
        path: String,
        destination: String,
    },
}
impl Service {
    pub fn asset_tools(&self, request: AssetRequest) -> Result<Value> {
        match request {
            AssetRequest::Inspect { project } => {
                let p = self.snapshot(project)?;
                let w = self.project_workspace(project);
                Ok(
                    json!({"project":project,"revision":p.revision,"assets":p.assets.iter().map(|a|{let file=w.path(&a.path);let metadata=file.ok().and_then(|p|p.metadata().ok());json!({"id":a.id,"name":a.name,"kind":a.kind,"path":a.path,"available":metadata.as_ref().is_some_and(|m|m.is_file()),"bytes":metadata.map(|m|m.len())})}).collect::<Vec<_>>(),"embedded_bitmaps":p.bitmaps.len()}),
                )
            }
            AssetRequest::Relink {
                project,
                expected_revision,
                asset,
                path,
            } => {
                let p = self.checked_project(project, expected_revision)?;
                let mut replacement = p
                    .assets
                    .iter()
                    .find(|a| a.id == asset)
                    .context("Asset missing")?
                    .clone();
                let w = self.project_workspace(project);
                let file = w.path(&path)?;
                ensure!(
                    file.is_file()
                        && file.metadata()?.len() <= astral_storage::MAX_PROJECT_BYTES as u64,
                    "Replacement must exist and fit 256 MiB"
                );
                if p.audio.iter().any(|c| c.asset == asset) {
                    astral_audio::decode(&w.read(&path)?)?;
                }
                replacement.path = path;
                let mut result = self.request(Request::ApplyCommands {
                    batch: Batch {
                        project,
                        expected_revision,
                        commands: vec![Command::ReplaceAsset { asset: replacement }],
                    },
                })?;
                result["warnings"] = json!([
                    "Source-file relinking does not replace embedded raster pixels. Re-import artwork when its content changes. Audio clips keep their timing; review trims against the replacement."
                ]);
                Ok(result)
            }
            AssetRequest::Package {
                project,
                expected_revision,
                path,
            } => {
                let p = self.checked_project(project, expected_revision)?;
                let w = self.project_workspace(project);
                self.asset_job(project, p.revision, move |job| {
                    w.package_project(&p, &path, || job.check())?;
                    Ok(json!({"path":path,"revision":p.revision}))
                })
            }
            AssetRequest::Unpack { path, destination } => {
                let w = self.workspace.clone();
                self.asset_job(Id::nil(), 0, move |job| {
                    let path = w.unpack_project(&path, &destination, || job.check())?;
                    Ok(json!({"path":path,"open_with":"open_project"}))
                })
            }
        }
    }
    fn asset_job(
        &self,
        project: Id,
        revision: u64,
        run: impl FnOnce(&crate::export::Job) -> Result<Value> + Send + 'static,
    ) -> Result<Value> {
        let mut store = self
            .store
            .lock()
            .map_err(|_| anyhow::anyhow!("Service unavailable"))?;
        store.jobs.retain(|_, j| !j.finished());
        ensure!(store.jobs.len() < 8, "Too many active jobs");
        let job = crate::export::background(project, revision, run);
        let id = job.id;
        store.jobs.insert(id, job);
        Ok(json!({"job":id}))
    }
}
