use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;

use crate::file::{BlockId, FileManager};
use crate::log::log::LogManager;
use crate::page::Page;

const PIN_TIME_LIMIT_IN_MILLIS: u128 = 5_000;

pub struct Buffer {
    file_manager: Arc<Mutex<FileManager>>,
    log_manager: Arc<Mutex<LogManager>>,
    pub page: Page,
    num_pins: usize,
    pub block: Option<BlockId>,
    modifying_transaction_id: Option<usize>,
    log_sequence_number: usize,
}

#[derive(Error, Debug)]
pub struct BufferAbortError {}

impl std::fmt::Display for BufferAbortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BufferAbortError")
    }
}

/// Buffer is a struct that represents a buffer in the buffer pool.
/// Each page in the buffer pool has information about whether it is pinned and which block it is assigned to.
impl Buffer {
    fn new(file_manager: Arc<Mutex<FileManager>>, log_manager: Arc<Mutex<LogManager>>) -> Self {
        let block_size = file_manager.lock().unwrap().block_size;
        Buffer {
            file_manager,
            log_manager,
            page: Page::new(block_size),
            num_pins: 0,
            block: None,
            modifying_transaction_id: None,
            log_sequence_number: 0,
        }
    }

    pub fn set_modified(&mut self, transaction_id: usize, log_sequence_number: usize) {
        self.modifying_transaction_id = Some(transaction_id);
        self.log_sequence_number = log_sequence_number;
    }

    fn is_pinned(&self) -> bool {
        self.num_pins > 0
    }

    fn assign_to_block(&mut self, block: &BlockId) -> Result<(), std::io::Error> {
        self.flush()?;
        self.block = Some(block.clone());
        self.file_manager
            .lock()
            .unwrap()
            .read(&block, &mut self.page)?;
        self.num_pins = 0;
        Ok(())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        // flush log and page if transaction_id is set
        if self.modifying_transaction_id.is_some() {
            let mut log_manager = self.log_manager.lock().unwrap();
            log_manager.flush(self.log_sequence_number)?;
            self.modifying_transaction_id = None;
        }
        Ok(())
    }

    fn pin(&mut self) {
        self.num_pins += 1;
    }

    fn unpin(&mut self) {
        self.num_pins -= 1;
    }
}

pub struct BufferManager {
    pub buffers: Mutex<Vec<Buffer>>,
    num_availables: usize,
    condvar: Condvar,
}

fn find_existing_buffer(buffers: &Vec<Buffer>, block: &BlockId) -> Option<usize> {
    for i in 0..buffers.len() {
        if let Some(b) = &buffers[i].block {
            if b == block {
                return Some(i);
            }
        }
    }
    None
}

fn choose_unpinned_buffer(buffers: &Vec<Buffer>) -> Option<usize> {
    for i in 0..buffers.len() {
        if !buffers[i].is_pinned() {
            return Some(i);
        }
    }
    None
}

fn try_to_pin(
    buffers: &mut Vec<Buffer>,
    num_availables: &mut usize,
    block: &BlockId,
) -> Result<Option<usize>, std::io::Error> {
    if let Some(buffer_index) = find_existing_buffer(buffers, block) {
        if !buffers[buffer_index].is_pinned() {
            *num_availables -= 1;
        }
        buffers[buffer_index].pin();
        return Ok(Some(buffer_index));
    }

    if let Some(buffer_index) = choose_unpinned_buffer(buffers) {
        buffers[buffer_index].assign_to_block(block)?;
        if !buffers[buffer_index].is_pinned() {
            *num_availables -= 1;
        }
        buffers[buffer_index].pin();
        Ok(Some(buffer_index))
    } else {
        Ok(None)
    }
}

impl BufferManager {
    pub fn new(
        file_manager: Arc<Mutex<FileManager>>,
        log_manager: Arc<Mutex<LogManager>>,
        num_buffers: usize,
    ) -> Self {
        let mut buffers = Vec::new();
        for _ in 0..num_buffers {
            buffers.push(Buffer::new(file_manager.clone(), log_manager.clone()));
        }

        BufferManager {
            buffers: Mutex::new(buffers),
            num_availables: num_buffers,
            condvar: Condvar::new(),
        }
    }

    pub fn get_num_availables(&self) -> usize {
        self.num_availables
    }

    pub fn unpin(&mut self, buffer_index: usize) {
        let mut buffers = self.buffers.lock().unwrap();
        buffers[buffer_index].unpin();
        if !buffers[buffer_index].is_pinned() {
            self.num_availables += 1;
        }
        self.condvar.notify_all();
    }

    pub fn pin(&mut self, block: &BlockId) -> Result<usize, anyhow::Error> {
        let timestamp = Instant::now();
        while timestamp.elapsed().as_millis() < PIN_TIME_LIMIT_IN_MILLIS {
            let mut buffers = self.buffers.lock().unwrap();
            if let Some(buffer_index) = try_to_pin(&mut buffers, &mut self.num_availables, block)? {
                return Ok(buffer_index);
            } else {
                let _lock = self
                    .condvar
                    .wait_timeout(
                        buffers,
                        Duration::new((PIN_TIME_LIMIT_IN_MILLIS / 1000) as u64, 0),
                    )
                    .unwrap();
            }
        }
        Err(anyhow::Error::new(BufferAbortError {}))
    }

    pub fn flush_all(&self, transaction_id: usize) -> Result<(), std::io::Error> {
        let mut buffers = self.buffers.lock().unwrap();
        for buffer in buffers.iter_mut() {
            if buffer.modifying_transaction_id == Some(transaction_id) {
                buffer.flush()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_manager() -> Result<(), anyhow::Error> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;

        let file_manager = Arc::new(Mutex::new(FileManager::new(temp_dir, block_size)));
        let log_manager = LogManager::new(file_manager.clone(), "log".into())?;
        let log_manager = Arc::new(Mutex::new(log_manager));
        let mut buffer_manager = BufferManager::new(file_manager.clone(), log_manager.clone(), 3);

        let block0 = file_manager.lock().unwrap().append_block("log")?;
        let block1 = file_manager.lock().unwrap().append_block("log")?;
        let block2 = file_manager.lock().unwrap().append_block("log")?;

        buffer_manager.pin(&block0)?;
        buffer_manager.pin(&block1)?;
        buffer_manager.pin(&block2)?;
        assert_eq!(buffer_manager.num_availables, 0);

        Ok(())
    }
}
