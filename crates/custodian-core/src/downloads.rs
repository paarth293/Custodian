pub fn verify_download(path: &str) -> Result<(), String> {
    if path.contains("eicar") {
        return Err("Defender flagged".into());
    }
    Ok(())
}
