use std::fs::File;
use std::io::{Read, Write, Result};
use std::path::Path;
use fs2::FileExt;
use serde::{Serialize, de::DeserializeOwned};
use tempfile::NamedTempFile;

pub fn read_json<T: DeserializeOwned>(path: &str) -> Result<T> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let data = serde_json::from_str(&contents)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(data)
}

pub fn atomic_write_json<T: Serialize>(path: &str, data: &T) -> Result<()> {
    let dir = Path::new(path).parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = NamedTempFile::new_in(dir)?;
    
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    
    tmp.write_all(json.as_bytes())?;
    tmp.flush()?;
    tmp.as_file().sync_all()?;
    
    tmp.persist(path).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    Ok(())
}

pub struct FileLock {
    file: File,
}

impl FileLock {
    pub fn new(path: &str) -> Result<Self> {
        let lock_path = format!("{}.lock", path);
        let file = File::create(lock_path)?;
        file.lock_exclusive()?;
        Ok(Self { file })
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}
