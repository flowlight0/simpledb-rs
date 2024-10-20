use std::fs::File;
use std::io::{Read, Result, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex, RwLock};
use std::{collections::HashMap, path::PathBuf};

use crate::{block_id::BlockId, page::Page};

pub struct FileManager {
    directory: PathBuf,
    block_size: usize,
    opened_files: RwLock<HashMap<String, Arc<Mutex<File>>>>,
}

impl FileManager {
    pub fn new(directory: PathBuf, block_size: usize) -> Self {
        if !directory.exists() {
            std::fs::create_dir_all(&directory).unwrap();
        }

        FileManager {
            directory,
            block_size,
            opened_files: RwLock::new(HashMap::new()),
        }
    }

    pub fn write(&mut self, block_id: &BlockId, page: &Page) -> Result<()> {
        let binding = self.load_and_cache_file(block_id);
        let mut file = binding.lock().unwrap();
        file.seek(SeekFrom::Start((block_id.block_slot * self.block_size) as u64))?;
        file.write(page.byte_buffer.as_slice())?;
        Ok(())
    }

    pub fn read(&mut self, block_id: &BlockId, page: &mut Page) -> Result<()> {
        let binding = self.load_and_cache_file(block_id);
        let mut file = binding.lock().unwrap();
        file.seek(SeekFrom::Start((block_id.block_slot * self.block_size) as u64))?;
        file.read_exact(&mut page.byte_buffer)?;
        Ok(())
    }

    fn load_and_cache_file(&mut self, block_id: &BlockId) -> Arc<Mutex<File>> {
        if let Some(file) = self
            .opened_files
            .try_read()
            .unwrap()
            .get(block_id.file_name)
        {
            return file.clone();
        }

        // Check if the file is added before acquiring the write lock
        let mut hash_map = self.opened_files.try_write().unwrap();
        if hash_map.contains_key(block_id.file_name) {
            return hash_map.get(block_id.file_name).unwrap().clone();
        }

        let file_path = self.directory.join(block_id.file_name);
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
             .open(file_path)
            .unwrap();
        let value = Arc::new(Mutex::new(file));
        hash_map.insert(block_id.file_name.to_string(), value.clone());
        return value;
    }
}

#[cfg(test)]
mod tests {
    use crate::{block_id::BlockId, page::Page};

    use super::*;
    use tempfile;

    #[test]
    fn test_simple_write_read() -> Result<()> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let mut file_manager = FileManager::new(temp_dir, block_size);
        let block_id = BlockId::new("testfile", 2);

        let mut page0 = Page::new(file_manager.block_size);
        page0.set_i32(80, 100);
        file_manager.write(&block_id, &page0)?;

        let mut page1 = Page::new(file_manager.block_size);
        file_manager.read(&block_id, &mut page1)?;
        assert_eq!(page1.get_i32(80), 100);
        Ok(())
    }
}
