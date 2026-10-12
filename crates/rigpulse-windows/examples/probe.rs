fn main() -> anyhow::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Provide an output path"))?;
    let inventory = rigpulse_windows::inventory::collect()?.private(false);
    let battery = rigpulse_windows::battery::read()?;
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&serde_json::json!({"inventory":inventory,"battery":battery}))?,
    )?;
    Ok(())
}
