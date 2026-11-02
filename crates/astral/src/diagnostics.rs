//! Local bounded panic reports. Never log panic payloads, project text or credentials.
use std::sync::Once;
static INSTALLED: Once = Once::new();
pub fn directory() -> std::path::PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("HOME"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Astral/diagnostics")
}
pub fn install() {
    INSTALLED.call_once(||{std::panic::set_hook(Box::new(|info|{
 let _=(||->std::io::Result<()>{let directory=directory();std::fs::create_dir_all(&directory)?;let when=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();let location=info.location().map(|l|format!("{}:{}:{}",std::path::Path::new(l.file()).file_name().unwrap_or_default().to_string_lossy(),l.line(),l.column())).unwrap_or_else(||"unknown".into());let trace=std::backtrace::Backtrace::force_capture().to_string();let mut text=format!("Astral {}\nPlatform {} {}\nLocation {}\nPanic payload omitted. Local report; never uploaded automatically.\n{}",env!("CARGO_PKG_VERSION"),std::env::consts::OS,std::env::consts::ARCH,location,trace);if text.len()>32768{let end=text.char_indices().take_while(|(i,_)|*i<=32768).last().map_or(0,|(i,_)|i);text.truncate(end);}std::fs::write(directory.join(format!("panic-{when}.txt")),text)?;let mut reports=std::fs::read_dir(&directory)?.filter_map(|e|e.ok()).filter(|e|e.file_type().is_ok_and(|t|t.is_file())&&e.file_name().to_string_lossy().starts_with("panic-")).collect::<Vec<_>>();reports.sort_by_key(|e|e.file_name());let remove=reports.len().saturating_sub(10);for report in reports.into_iter().take(remove){let _=std::fs::remove_file(report.path());}Ok(())})();
 }));});
}
