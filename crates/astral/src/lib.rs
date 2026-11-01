pub mod assistance;
#[cfg(feature = "desktop")]
pub mod desktop;
pub mod export;
pub mod ipc;
pub mod mcp;
pub mod projects;
pub mod service;
pub mod voice;

pub fn default_workspace() -> std::path::PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("Documents/Astral")
}

pub mod production;
