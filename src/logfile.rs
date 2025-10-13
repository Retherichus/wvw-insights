use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LogFile {
    pub path: PathBuf,
    pub filename: String,
    pub size: u64,
    pub modified: u64,
    pub selected: bool,
    pub uploaded: bool,
    pub status: String,
}

impl LogFile {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let metadata = std::fs::metadata(&path)?;
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        
        let modified = metadata
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        Ok(Self {
            path,
            filename,
            size: metadata.len(),
            modified,
            selected: false,
            uploaded: false,
            status: "Ready".to_string(),
        })
    }
}