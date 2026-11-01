fn main() -> anyhow::Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let started = std::time::Instant::now();
    let (model, mut report) = astral_assist::network::train(8192, 100, 0x41535452414c)?;
    report["training_seconds"] = started.elapsed().as_secs_f64().into();
    std::fs::write(
        root.join("models/pose-prior-v1.json"),
        serde_json::to_vec(&model)?,
    )?;
    std::fs::write(
        root.join("models/pose-prior-v1-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
