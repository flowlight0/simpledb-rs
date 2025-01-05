use std::fs::{read_dir, remove_file, File};
use std::io::{Result, Seek, SeekFrom, Write};

use std::sync::{Arc, Mutex, RwLock};
use std::{collections::HashMap, path::PathBuf};

use crate::page::Page;

const DUMMY_BLOCK_SIZE: usize = usize::MAX;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockId {
    pub file_name: String,
    pub block_slot: usize,
}

impl BlockId {
    pub fn new(file_name: &str, block_slot: usize) -> Self {
        BlockId {
            file_name: file_name.to_string(),
            block_slot,
        }
    }

    pub fn create_dummy(file_name: &str) -> Self {
        BlockId {
            file_name: file_name.to_string(),
            block_slot: DUMMY_BLOCK_SIZE,
        }
    }

    pub fn get_first_block(file_name: &str) -> Self {
        BlockId {
            file_name: file_name.to_string(),
            block_slot: 0,
        }
    }

    pub fn get_previous_block(&self) -> Option<BlockId> {
        if self.block_slot == 0 {
            return None;
        }
        Some(BlockId {
            file_name: self.file_name.clone(),
            block_slot: self.block_slot - 1,
        })
    }

    pub fn get_next_block(&self) -> BlockId {
        BlockId {
            file_name: self.file_name.clone(),
            block_slot: self.block_slot + 1,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let file_name_bytes = self.file_name.as_bytes();
        let file_name_length = file_name_bytes.len();

        let block_slot_bytes = self.block_slot.to_le_bytes().to_vec();
        assert_eq!(block_slot_bytes.len(), 8);

        let total_length = 8 + 8 + file_name_length;
        let mut bytes = vec![];
        bytes.extend(total_length.to_le_bytes());
        bytes.extend(block_slot_bytes);
        bytes.extend(file_name_bytes);
        assert_eq!(bytes.len(), total_length);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> (usize, Self) {
        let total_length = usize::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        let block_slot = usize::from_le_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]);

        let file_name = String::from_utf8(bytes[16..total_length].to_vec()).unwrap();
        (
            total_length,
            BlockId {
                file_name,
                block_slot,
            },
        )
    }
}

pub struct FileAccessStats {
    pub read_count: usize,
    pub write_count: usize,
}

impl FileAccessStats {
    pub fn new() -> Self {
        FileAccessStats {
            read_count: 0,
            write_count: 0,
        }
    }

    pub fn reset(&mut self) {
        self.read_count = 0;
        self.write_count = 0;
    }
}

pub struct FileManager {
    directory: PathBuf,
    pub block_size: usize,
    opened_files: RwLock<HashMap<String, Arc<Mutex<File>>>>,
    pub is_new: bool,
    pub file_access_stats: FileAccessStats,
}

impl FileManager {
    pub fn new(directory: PathBuf, block_size: usize) -> Self {
        let is_new = if !directory.exists() {
            std::fs::create_dir_all(&directory).unwrap();
            true
        } else {
            false
        };

        for entry in read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() {
                remove_file(path).unwrap();
            }
        }

        FileManager {
            directory,
            block_size,
            opened_files: RwLock::new(HashMap::new()),
            is_new,
            file_access_stats: FileAccessStats::new(),
        }
    }

    pub fn write(&mut self, block: &BlockId, page: &Page) -> Result<()> {
        let binding = self.load_and_cache_file(&block.file_name);
        let mut file = binding.lock().unwrap();
        file.seek(SeekFrom::Start((block.block_slot * self.block_size) as u64))?;
        page.write_to_file(&mut file)?;
        self.file_access_stats.write_count += 1;
        Ok(())
    }

    pub fn read(&mut self, block: &BlockId, page: &mut Page) -> Result<()> {
        let binding = self.load_and_cache_file(&block.file_name);
        let mut file = binding.lock().unwrap();
        file.seek(SeekFrom::Start((block.block_slot * self.block_size) as u64))?;
        page.read_from_file(&mut file)?;
        self.file_access_stats.read_count += 1;
        Ok(())
    }

    pub fn get_last_block(&mut self, file_name: &str) -> BlockId {
        let num_blocks = self.get_num_blocks(file_name);
        BlockId::new(file_name, num_blocks - 1)
    }

    pub fn get_num_blocks(&mut self, file_name: &str) -> usize {
        let binding = self.load_and_cache_file(file_name);
        let file = binding.lock().unwrap();
        file.metadata().unwrap().len() as usize / self.block_size
    }

    pub fn append_block(&mut self, file_name: &str) -> Result<BlockId> {
        let binding = self.load_and_cache_file(file_name);
        let mut file = binding.lock().unwrap();
        let num_blocks = file.metadata().unwrap().len() as usize / self.block_size;
        let new_block_contents = vec![0; self.block_size];
        file.seek(SeekFrom::End(0))?;
        file.write(new_block_contents.as_slice())?;
        self.file_access_stats.write_count += 1;
        Ok(BlockId::new(file_name, num_blocks))
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

        let file_path = self.directory.join(file_name);
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

        assert_eq!(file_manager.file_access_stats.write_count, 2); // append + write
        assert_eq!(file_manager.file_access_stats.read_count, 1);
        Ok(())
    }

    #[test]
    fn test_append_blocks() -> Result<()> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let mut file_manager = FileManager::new(temp_dir, block_size);

        assert_eq!(file_manager.get_num_blocks("testfile"), 0);
        for i in 0..10 {
            let block = file_manager.append_block("testfile")?;
            assert_eq!(block, BlockId::new("testfile", i));
            assert_eq!(file_manager.get_num_blocks("testfile"), i + 1);
        }
        assert_eq!(file_manager.file_access_stats.write_count, 10);
        assert_eq!(file_manager.file_access_stats.read_count, 0);
        Ok(())
    }
}
