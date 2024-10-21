use std::fs::File;
use std::io::{Read, Result, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex, RwLock};
use std::{collections::HashMap, path::PathBuf};

use crate::page::Page;

type FileId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId {
    pub file_id: FileId,
    pub block_slot: usize,
}

impl BlockId {
    pub fn new(file_id: usize, block_slot: usize) -> Self {
        BlockId {
            file_id,
            block_slot,
        }
    }
}

pub struct FileManager {
    directory: PathBuf,
    pub block_size: usize,
    opened_files: RwLock<HashMap<usize, Arc<Mutex<File>>>>,

    file_id_mapping_lock: RwLock<()>,
    file_id_to_file_name: HashMap<FileId, String>,
    file_name_to_file_id: HashMap<String, FileId>,
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
            file_id_mapping_lock: RwLock::new(()),
            file_id_to_file_name: HashMap::new(),
            file_name_to_file_id: HashMap::new(),
        }
    }

    pub fn write(&mut self, block_id: &BlockId, page: &Page) -> Result<()> {
        let binding = self.load_and_cache_file(block_id.file_id);
        let mut file = binding.lock().unwrap();
        file.seek(SeekFrom::Start(
            (block_id.block_slot * self.block_size) as u64,
        ))?;
        file.write(page.byte_buffer.as_slice())?;
        Ok(())
    }

    pub fn read(&mut self, block_id: &BlockId, page: &mut Page) -> Result<()> {
        let binding = self.load_and_cache_file(block_id.file_id);
        let mut file = binding.lock().unwrap();
        file.seek(SeekFrom::Start(
            (block_id.block_slot * self.block_size) as u64,
        ))?;
        file.read_exact(&mut page.byte_buffer)?;
        Ok(())
    }

    // This method is not expcted to be concurrently called
    pub fn get_last_block(&mut self, file_name: &str) -> BlockId {
        let file_id = self.get_file_id(file_name);
        let num_blocks = self.get_num_blocks(file_name);
        BlockId::new(file_id, num_blocks - 1)
    }

    pub fn get_num_blocks(&mut self, file_name: &str) -> usize {
        let file_id = self.get_file_id(file_name);
        let binding = self.load_and_cache_file(file_id);
        let file = binding.lock().unwrap();
        file.metadata().unwrap().len() as usize / self.block_size
    }

    pub fn append_block(&mut self, file_name: &str) -> Result<BlockId> {
        let file_id = self.get_file_id(file_name);
        let binding = self.load_and_cache_file(file_id);
        let mut file = binding.lock().unwrap();
        let num_blocks = file.metadata().unwrap().len() as usize / self.block_size;
        let new_block_contents = vec![0; self.block_size];
        file.write(new_block_contents.as_slice())?;
        Ok(BlockId::new(file_id, num_blocks))
    }

    fn load_and_cache_file(&mut self, file_id: FileId) -> Arc<Mutex<File>> {
        if let Some(file) = self.opened_files.try_read().unwrap().get(&file_id) {
            return file.clone();
        }

        // Check if the file is added before acquiring the write lock
        let mut hash_map = self.opened_files.try_write().unwrap();
        if hash_map.contains_key(&file_id) {
            return hash_map.get(&file_id).unwrap().clone();
        }

        let file_name = self.get_file_name(file_id);
        let file_path = self.directory.join(file_name);
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(file_path)
            .unwrap();
        let value = Arc::new(Mutex::new(file));
        hash_map.insert(file_id, value.clone());
        return value;
    }

    pub fn get_file_id(&mut self, file_name: &str) -> FileId {
        {
            let _lookup_lock = self.file_id_mapping_lock.read().unwrap();
            if let Some(file_id) = self.file_name_to_file_id.get(file_name) {
                return *file_id;
            }
        }

        let _update_lock = self.file_id_mapping_lock.write().unwrap();
        if let Some(file_id) = self.file_name_to_file_id.get(file_name) {
            return *file_id;
        }

        let file_id = self.file_name_to_file_id.len();
        self.file_name_to_file_id
            .insert(file_name.to_string(), file_id);
        self.file_id_to_file_name
            .insert(file_id, file_name.to_string());
        file_id
    }

    fn get_file_name(&self, file_id: usize) -> &str {
        let _lock = self.file_id_mapping_lock.read().unwrap();
        return self
            .file_id_to_file_name
            .get(&file_id)
            .expect("File ID must be registered before we reach here");
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

        let block_id = file_manager.append_block("testfile")?;
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
            let block_id = file_manager.append_block("testfile")?;
            assert_eq!(block_id, BlockId::new(0, i));
            assert_eq!(file_manager.get_num_blocks("testfile"), i + 1);
        }
        Ok(())
    }
}
