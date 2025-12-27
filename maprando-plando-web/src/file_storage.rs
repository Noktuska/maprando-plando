use std::path::Path;

use anyhow::Result;
use futures::future::join_all;
use object_store::{ObjectStore, ObjectStoreExt, local::LocalFileSystem, memory::InMemory};

pub struct SeedFile {
    pub name: String,
    pub data: Vec<u8>
}

pub struct Seed {
    pub seed_id: String,
    pub files: Vec<SeedFile>
}

pub struct FileStorage {
    object_store: Box<dyn ObjectStore>,
    base_path: String
}

impl FileStorage {
    pub fn new(path: &str) -> Self {
        let object_store: Box<dyn ObjectStore> = if path == "mem" {
            Box::new(InMemory::new())
        } else if let Some(root) = path.strip_prefix("file:") {
            Box::new(LocalFileSystem::new_with_prefix(Path::new(root)).expect("Unable to create LocalFileSystem"))
        } else {
            panic!("Invalid file storage: {path}")
        };
        Self {
            object_store,
            base_path: "seeds/".to_string()
        }
    }

    pub async fn get_file(&self, path: String) -> Result<Vec<u8>> {
        let full_path = self.base_path.clone() + &path;
        let path = object_store::path::Path::parse(full_path)?;
        let file = self.object_store.get(&path).await?.bytes().await?;
        Ok(file.to_vec())
    }

    pub async fn put_file(&self, path: String, data: Vec<u8>) -> Result<()> {
        let full_path = self.base_path.clone() + &path;
        let path = object_store::path::Path::parse(full_path)?;
        self.object_store.put(&path, data.into()).await?;
        Ok(())
    }

    pub async fn put_seed(&self, seed: Seed) -> Result<()> {
        let mut futures = Vec::with_capacity(seed.files.len());
        for file in seed.files {
            let path = format!("{}/{}", seed.seed_id, file.name);
            futures.push(self.put_file(path, file.data));
        }
        let results = join_all(futures).await;
        for result in results {
            result?;
        }
        Ok(())
    }
}