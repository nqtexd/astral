//! Local desktop bridge. No TCP listener or exposed shell execution.
use crate::service::{Request, Service};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::{
    hash::{Hash, Hasher},
    path::Path,
};
#[cfg(windows)]
use tokio::io::AsyncBufReadExt;
use tokio::io::{AsyncWriteExt, BufReader};

pub fn pipe_name(root: &Path) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    root.to_string_lossy().to_lowercase().hash(&mut hasher);
    format!(r"\\.\pipe\astral-{:016x}", hasher.finish())
}

#[cfg(windows)]
pub async fn serve(service: Service) -> Result<()> {
    use tokio::net::windows::named_pipe::ServerOptions;
    let name = pipe_name(service.workspace.root());
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(&name)?;
    loop {
        server.connect().await?;
        let connection = server;
        server = ServerOptions::new()
            .reject_remote_clients(true)
            .create(&name)?;
        let service = service.clone();
        tokio::spawn(async move {
            let mut stream = BufReader::new(connection);
            let mut bytes = vec![];
            let result = async {
                use tokio::io::AsyncReadExt;
                loop {
                    let byte =
                        tokio::time::timeout(std::time::Duration::from_secs(30), stream.read_u8())
                            .await??;
                    if byte == b'\n' {
                        break;
                    }
                    ensure!(bytes.len() < 4 * 1024 * 1024, "IPC request exceeds 4 MiB");
                    bytes.push(byte);
                }
                let request: Request = serde_json::from_slice(&bytes)?;
                tokio::task::spawn_blocking(move || service.request(request)).await?
            }
            .await;
            let response = match result {
                Ok(v) => json!({"ok":true,"result":v}),
                Err(e) => json!({"ok":false,"error":format!("{e:#}")}),
            };
            if let Ok(mut bytes) = serde_json::to_vec(&response) {
                bytes.push(b'\n');
                let _ = stream.get_mut().write_all(&bytes).await;
            }
        });
    }
}

#[cfg(windows)]
pub async fn call(root: &Path, request: Request) -> Result<Value> {
    use tokio::net::windows::named_pipe::ClientOptions;
    let stream = ClientOptions::new().open(pipe_name(root))?;
    let mut stream = BufReader::new(stream);
    let mut bytes = serde_json::to_vec(&request)?;
    ensure!(bytes.len() < 4 * 1024 * 1024, "IPC request exceeds 4 MiB");
    bytes.push(b'\n');
    stream.get_mut().write_all(&bytes).await?;
    let mut response = String::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(120),
        stream.read_line(&mut response),
    )
    .await??;
    ensure!(
        response.len() <= 512 * 1024 * 1024,
        "IPC response too large"
    );
    let value: Value = serde_json::from_str(&response)?;
    ensure!(
        value["ok"] == true,
        "{}",
        value["error"].as_str().unwrap_or("Desktop request failed")
    );
    Ok(value["result"].clone())
}
#[cfg(unix)]
mod unix {
    use super::*;
    use std::{
        io::ErrorKind,
        os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt},
        path::PathBuf,
    };
    use tokio::net::{UnixListener, UnixStream};
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    fn uid() -> u32 {
        unsafe { geteuid() }
    }
    pub(super) fn socket(root: &Path) -> Result<PathBuf> {
        let user = uid();
        let directory = std::env::temp_dir().join(format!("astral-ipc-{user}"));
        match std::fs::DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
        let meta = std::fs::symlink_metadata(&directory)?;
        ensure!(
            meta.is_dir()
                && !meta.file_type().is_symlink()
                && meta.uid() == user
                && meta.permissions().mode() & 0o077 == 0,
            "IPC directory must be private and owned by the current user"
        );
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        root.hash(&mut hash);
        let path = directory.join(format!("{:016x}.sock", hash.finish()));
        ensure!(path.as_os_str().len() < 104, "Unix socket path too long");
        Ok(path)
    }
    struct Cleanup {
        path: PathBuf,
        inode: u64,
    }
    impl Drop for Cleanup {
        fn drop(&mut self) {
            if std::fs::symlink_metadata(&self.path).is_ok_and(|m| {
                m.ino() == self.inode && m.file_type().is_socket() && m.uid() == uid()
            }) {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
    async fn line<R: tokio::io::AsyncRead + Unpin>(reader: &mut R, max: usize) -> Result<Vec<u8>> {
        use tokio::io::AsyncReadExt;
        let mut bytes = Vec::new();
        loop {
            let byte = reader.read_u8().await?;
            if byte == b'\n' {
                return Ok(bytes);
            }
            ensure!(bytes.len() < max, "IPC message exceeds limit");
            bytes.push(byte);
        }
    }
    pub(super) async fn serve(service: Service) -> Result<()> {
        let path = socket(service.workspace.root())?;
        if let Ok(meta) = std::fs::symlink_metadata(&path) {
            ensure!(
                meta.file_type().is_socket() && meta.uid() == uid(),
                "Refusing to replace an unowned/non-socket IPC path"
            );
            match UnixStream::connect(&path).await {
                Ok(_) => {
                    anyhow::bail!("Astral desktop bridge is already running for this workspace")
                }
                Err(e)
                    if matches!(e.kind(), ErrorKind::ConnectionRefused | ErrorKind::NotFound) =>
                {
                    std::fs::remove_file(&path)?
                }
                Err(e) => return Err(e.into()),
            }
        }
        let listener = UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        let _cleanup = Cleanup {
            inode: std::fs::symlink_metadata(&path)?.ino(),
            path,
        };
        loop {
            let (stream, _) = listener.accept().await?;
            if stream.peer_cred()?.uid() != uid() {
                continue;
            }
            let service = service.clone();
            tokio::spawn(async move {
                let mut stream = BufReader::new(stream);
                let result = async {
                    let bytes = tokio::time::timeout(
                        std::time::Duration::from_secs(30),
                        line(&mut stream, 4 * 1024 * 1024),
                    )
                    .await??;
                    let request: Request = serde_json::from_slice(&bytes)?;
                    tokio::task::spawn_blocking(move || service.request(request)).await?
                }
                .await;
                let response = match result {
                    Ok(v) => json!({"ok":true,"result":v}),
                    Err(e) => json!({"ok":false,"error":format!("{e:#}")}),
                };
                if let Ok(mut bytes) = serde_json::to_vec(&response) {
                    bytes.push(b'\n');
                    let _ = stream.get_mut().write_all(&bytes).await;
                }
            });
        }
    }
    pub(super) async fn call(root: &Path, request: Request) -> Result<Value> {
        let stream = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            UnixStream::connect(socket(root)?),
        )
        .await??;
        ensure!(
            stream.peer_cred()?.uid() == uid(),
            "Desktop bridge belongs to another user"
        );
        let mut stream = BufReader::new(stream);
        let mut bytes = serde_json::to_vec(&request)?;
        ensure!(bytes.len() < 4 * 1024 * 1024, "IPC request exceeds 4 MiB");
        bytes.push(b'\n');
        stream.get_mut().write_all(&bytes).await?;
        let bytes = tokio::time::timeout(
            std::time::Duration::from_secs(120),
            line(&mut stream, 512 * 1024 * 1024),
        )
        .await??;
        let value: Value = serde_json::from_slice(&bytes)?;
        ensure!(
            value["ok"] == true,
            "{}",
            value["error"].as_str().unwrap_or("Desktop request failed")
        );
        Ok(value["result"].clone())
    }
}
#[cfg(unix)]
pub async fn serve(service: Service) -> Result<()> {
    unix::serve(service).await
}
#[cfg(unix)]
pub async fn call(root: &Path, request: Request) -> Result<Value> {
    unix::call(root, request).await
}

#[cfg(all(not(windows), not(unix)))]
pub async fn serve(_service: Service) -> Result<()> {
    anyhow::bail!("This target has no supported local desktop transport")
}
#[cfg(all(not(windows), not(unix)))]
pub async fn call(_root: &Path, _request: Request) -> Result<Value> {
    anyhow::bail!("This target has no supported local desktop transport")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[tokio::test]
    async fn private_unix_bridge_roundtrips_and_rejects_a_second_server() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path()).unwrap();
        let path = unix::socket(service.workspace.root()).unwrap();
        let other = service.clone();
        let task = tokio::spawn(serve(service.clone()));
        for _ in 0..100 {
            if path.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        let reply = call(service.workspace.root(), Request::ListProjects)
            .await
            .unwrap();
        assert_eq!(reply["projects"].as_array().unwrap().len(), 0);
        assert!(serve(other).await.is_err());
        task.abort();
        let _ = task.await;
    }
}
