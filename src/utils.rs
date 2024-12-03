use std::path::{Path, PathBuf};
use std::io::Write;
use std::fs;


pub fn create_dir(path: impl AsRef<Path>) -> std::io::Result<()> {
    fs::create_dir(path)
}

pub fn get_current_dir() -> std::io::Result<PathBuf> {
    std::env::current_dir()
}

pub fn write_to_file(path: impl AsRef<Path>, content: &str) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(content.as_bytes())?;
    file.flush()
}