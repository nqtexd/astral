fn main() -> anyhow::Result<()> {
    let root = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("AstralTouchPreview"));
    astral_mobile::run(root, eframe::NativeOptions::default())
}
