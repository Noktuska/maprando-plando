use std::{io::{Read, Write}, path::PathBuf};

use anyhow::Result;

pub trait FileStorage: Send + Sync {
    fn put_file(&self, name: &str, data: &[u8]) -> Result<()> ;
    fn get_file(&self, name: &str) -> Result<Vec<u8>>;
}

pub struct LocalFileStorage {
    pub path: PathBuf
}

impl FileStorage for LocalFileStorage {
    fn put_file(&self, name: &str, data: &[u8]) -> Result<()> {
        let path = self.path.join(name);
        let mut f = std::fs::File::create(path)?;
        f.write_all(data)?;
        Ok(())
    }

    fn get_file(&self, name: &str) -> Result<Vec<u8>> {
        let path = self.path.join(name);
        let mut f = std::fs::File::open(path)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        Ok(buf)
    }
}