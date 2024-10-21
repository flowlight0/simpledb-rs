use std::fs::File;
use std::io::{Read, Result, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex, RwLock};
use std::{collections::HashMap, path::PathBuf};

use crate::page::Page;

pub struct BlockId<'a> {
    pub file_name: &'a str,
    pub block_slot: usize,
}

impl<'a> BlockId<'a> {
    pub fn new(file_name: &'a str, block_slot: usize) -> Self {
        BlockId {
            file_name,
            block_slot,
        }
    }
}

pub struct FileManager {
    directory: PathBuf,
    pub block_size: usize,
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
        let binding = self.load_and_cache_file(block_id.file_name);
        let mut file = binding.lock().unwrap();
        file.seek(SeekFrom::Start(
            (block_id.block_slot * self.block_size) as u64,
        ))?;
        file.write(page.byte_buffer.as_slice())?;
        Ok(())
    }

    pub fn read(&mut self, block_id: &BlockId, page: &mut Page) -> Result<()> {
        let binding = self.load_and_cache_file(block_id.file_name);
        let mut file = binding.lock().unwrap();
        file.seek(SeekFrom::Start(
            (block_id.block_slot * self.block_size) as u64,
        ))?;
        file.read_exact(&mut page.byte_buffer)?;
        Ok(())
    }

    pub fn get_num_blocks(&mut self, file_name: &str) -> usize {
        let binding = self.load_and_cache_file(file_name);
        let file = binding.lock().unwrap();
        file.metadata().unwrap().len() as usize / self.block_size
    }

    pub fn append_block<'a>(&mut self, file_name: &'a str) -> Result<usize> {
        let binding = self.load_and_cache_file(file_name);
        let mut file = binding.lock().unwrap();
        let num_blocks = file.metadata().unwrap().len() as usize / self.block_size;
        let new_block_contents = vec![0; self.block_size];
        file.write(new_block_contents.as_slice())?;
        Ok(num_blocks)
    }

    fn load_and_cache_file(&mut self, file_name: &str) -> Arc<Mutex<File>> {
        if let Some(file) = self.opened_files.try_read().unwrap().get(file_name) {
            return file.clone();
        }

        // Check if the file is added before acquiring the write lock
        let mut hash_map = self.opened_files.try_write().unwrap();
        if hash_map.contains_key(file_name) {
            return hash_map.get(file_name).unwrap().clone();
        }

        let file_path = self.get_path(file_name);
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(file_path)
            .unwrap();
        let value = Arc::new(Mutex::new(file));
        hash_map.insert(file_name.to_string(), value.clone());
        return value;
    }

    fn get_path(&self, file_name: &str) -> PathBuf {
        self.directory.join(file_name)
    }
}

#[cfg(test)]
mod tests {
    use crate::page::Page;

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

    #[test]
    fn test_append_blocks() -> Result<()> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let mut file_manager = FileManager::new(temp_dir, block_size);

        assert_eq!(file_manager.get_num_blocks("testfile"), 0);
        for i in 0..10 {
            let block_slot = file_manager.append_block("testfile")?;
            assert_eq!(block_slot, i);
            assert_eq!(file_manager.get_num_blocks("testfile"), i + 1);
        }
        Ok(())
    }
}
