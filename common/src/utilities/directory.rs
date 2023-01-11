pub async fn ensure_directory_structure(base: &str, folder: &str) -> std::io::Result<()> {
    let base_dir = std::path::Path::new(base);
    let folder_dir = std::path::Path::new(folder);

    if folder_dir.is_absolute() && !folder_dir.exists() {
        tokio::fs::create_dir(folder_dir).await?;
    } else {
        let combined_dir = base_dir.join(folder_dir);
        if !combined_dir.exists() {
            tokio::fs::create_dir(combined_dir).await?;
        }
    }
    Ok(())
}
