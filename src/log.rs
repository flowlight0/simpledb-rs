use std::{
    io::Result,
    mem,
    sync::{Arc, Mutex, MutexGuard},
};

use crate::{
    file::{BlockId, FileManager},
    page::Page,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogRecord {
    Start(usize),
    Commit(usize),
    Checkpoint(usize),
    Rollback(usize),
    SetI32(usize, BlockId, usize, i32, i32),
}

impl LogRecord {
    fn to_bytes(&self) -> Vec<u8> {
        dbg!(&self);
        match self {
            LogRecord::Start(transaction_id) => {
                let mut bytes = Vec::new();
                bytes.push('S' as u8);
                bytes.extend_from_slice(&transaction_id.to_ne_bytes());
                bytes
            }
            LogRecord::Commit(transaction_id) => {
                let mut bytes = Vec::new();
                bytes.push('C' as u8);
                bytes.extend_from_slice(&transaction_id.to_ne_bytes());
                bytes
            }
            LogRecord::Checkpoint(transaction_id) => {
                let mut bytes = Vec::new();
                bytes.push('K' as u8);
                bytes.extend_from_slice(&transaction_id.to_ne_bytes());
                bytes
            }
            LogRecord::Rollback(transaction_id) => {
                let mut bytes = Vec::new();
                bytes.push('R' as u8);
                bytes.extend_from_slice(&transaction_id.to_ne_bytes());
                bytes
            }
            LogRecord::SetI32(transaction_id, block_id, offset, old_value, new_value) => {
                let mut bytes = Vec::new();
                bytes.push('I' as u8);
                bytes.extend_from_slice(&transaction_id.to_ne_bytes());
                bytes.extend_from_slice(&block_id.file_id.to_ne_bytes());
                bytes.extend_from_slice(&block_id.block_slot.to_ne_bytes());
                bytes.extend_from_slice(&offset.to_ne_bytes());
                bytes.extend_from_slice(&old_value.to_ne_bytes());
                bytes.extend_from_slice(&new_value.to_ne_bytes());
                bytes
            }
        }
    }

    pub fn log_into(&self, log_manager: &mut LogManager) -> Result<usize> {
        log_manager.append_record(self)
    }

    pub fn get_transaction_id(&self) -> usize {
        match self {
            LogRecord::Start(transaction_id) => *transaction_id,
            LogRecord::Commit(transaction_id) => *transaction_id,
            LogRecord::Checkpoint(transaction_id) => *transaction_id,
            LogRecord::Rollback(transaction_id) => *transaction_id,
            LogRecord::SetI32(transaction_id, _, _, _, _) => *transaction_id,
        }
    }

    fn from_bytes(current_position: &[u8]) -> Self {
        for i in 0..current_position.len() {
            dbg!(current_position[i] as char);
        }
        match current_position[0] as char {
            'S' => {
                let transaction_id = usize::from_ne_bytes([
                    current_position[1],
                    current_position[2],
                    current_position[3],
                    current_position[4],
                    current_position[5],
                    current_position[6],
                    current_position[7],
                    current_position[8],
                ]);
                LogRecord::Start(transaction_id)
            }
            'C' => {
                let transaction_id = usize::from_ne_bytes([
                    current_position[1],
                    current_position[2],
                    current_position[3],
                    current_position[4],
                    current_position[5],
                    current_position[6],
                    current_position[7],
                    current_position[8],
                ]);
                LogRecord::Commit(transaction_id)
            }
            'K' => {
                let transaction_id = usize::from_ne_bytes([
                    current_position[1],
                    current_position[2],
                    current_position[3],
                    current_position[4],
                    current_position[5],
                    current_position[6],
                    current_position[7],
                    current_position[8],
                ]);
                LogRecord::Checkpoint(transaction_id)
            }
            'R' => {
                let transaction_id = usize::from_ne_bytes([
                    current_position[1],
                    current_position[2],
                    current_position[3],
                    current_position[4],
                    current_position[5],
                    current_position[6],
                    current_position[7],
                    current_position[8],
                ]);
                LogRecord::Rollback(transaction_id)
            }
            'I' => {
                let transaction_id = usize::from_ne_bytes([
                    current_position[1],
                    current_position[2],
                    current_position[3],
                    current_position[4],
                    current_position[5],
                    current_position[6],
                    current_position[7],
                    current_position[8],
                ]);
                let file_id = usize::from_ne_bytes([
                    current_position[9],
                    current_position[10],
                    current_position[11],
                    current_position[12],
                    current_position[13],
                    current_position[14],
                    current_position[15],
                    current_position[16],
                ]);
                let block_slot = usize::from_ne_bytes([
                    current_position[17],
                    current_position[18],
                    current_position[19],
                    current_position[20],
                    current_position[21],
                    current_position[22],
                    current_position[23],
                    current_position[24],
                ]);
                let offset = usize::from_ne_bytes([
                    current_position[25],
                    current_position[26],
                    current_position[27],
                    current_position[28],
                    current_position[29],
                    current_position[30],
                    current_position[31],
                    current_position[32],
                ]);
                let old_value = i32::from_ne_bytes([
                    current_position[33],
                    current_position[34],
                    current_position[35],
                    current_position[36],
                ]);
                let new_value = i32::from_ne_bytes([
                    current_position[37],
                    current_position[38],
                    current_position[39],
                    current_position[40],
                ]);
                LogRecord::SetI32(
                    transaction_id,
                    BlockId::new(file_id, block_slot),
                    offset,
                    old_value,
                    new_value,
                )
            }
            _ => panic!("Invalid log record"),
        }
    }
}

/// The log manager is responsible for writing log records to the log file.
/// The log file is a sequence of blocks, and the log manager appends log records to the last block.
/// The log records are written to the log file from right to left.
/// The first byte of the block contains the offset to the most recent log record.
///
/// This struct is expected to be a singleton. Hence, it is not thread-safe.
/// Other structs that use this struct should wrap it with Arc<Mutex<>>.
pub struct LogManager {
    file_manager: Arc<Mutex<FileManager>>,
    log_file: String,
    log_page: Page,
    current_block: BlockId,
    append_lock: Mutex<()>,
    latest_log_sequence_number: usize,
    last_saved_log_sequence_number: usize,
}

impl LogManager {
    pub fn new(file_manager: Arc<Mutex<FileManager>>, log_file: String) -> Result<Self> {
        let mut file_manager_guard = file_manager.lock().unwrap();
        let num_blocks = file_manager_guard.get_num_blocks(&log_file);
        let mut log_page = Page::new(file_manager_guard.block_size);
        let current_block = if num_blocks == 0 {
            // Create the first block of the file
            let block_id = file_manager_guard.append_block(&log_file)?;

            // Currently, page cannot store usize values
            log_page.set_i32(0, file_manager_guard.block_size as i32);
            file_manager_guard.write(&block_id, &log_page)?;
            block_id
        } else {
            // Read the last block of the file
            let current_block = file_manager_guard.get_last_block(&log_file);
            file_manager_guard.read(&current_block, &mut log_page)?;
            current_block
        };

        Ok(Self {
            file_manager: file_manager.clone(),
            log_file,
            log_page,
            current_block,
            append_lock: Mutex::new(()),
            latest_log_sequence_number: 0,
            last_saved_log_sequence_number: 0,
        })
    }

    pub fn append_record(&mut self, log_record: &LogRecord) -> Result<usize> {
        let _lock = self.append_lock.lock().unwrap();
        let mut boundary = self.log_page.get_i32(0) as usize;
        let record_bytes = log_record.to_bytes();
        let record_size = record_bytes.len();
        let boundary_size = mem::size_of::<i32>();
        let mut file_manager = self.file_manager.lock().unwrap();

        if boundary < record_size + boundary_size {
            assert!(false);
            // The record does not fit in the current block

            // Save the current page into the file
            file_manager.write(&self.current_block, &self.log_page)?;
            self.last_saved_log_sequence_number = self.latest_log_sequence_number;

            // Create a new block
            let new_block = file_manager.append_block(&self.log_file)?;
            self.log_page.set_i32(0, file_manager.block_size as i32);
            file_manager.write(&new_block, &mut self.log_page)?;
            self.current_block = new_block;
            boundary = self.log_page.get_i32(0) as usize;
        }

        let record_position = boundary - record_size;
        dbg!(record_position, record_size, boundary);
        self.log_page
            .set_bytes(record_position, record_bytes.as_slice());
        self.log_page.set_i32(0, record_position as i32);
        self.latest_log_sequence_number += 1;
        Ok(self.latest_log_sequence_number)
    }

    // Flushes the log records up to the least_log_sequence_number.
    pub fn flush(&mut self, least_log_sequence_number: usize) -> Result<()> {
        if least_log_sequence_number >= self.last_saved_log_sequence_number {
            self.do_flush()?;
        }
        Ok(())
    }

    pub fn get_backward_iter(&mut self) -> Result<BackwardLogIterator> {
        self.do_flush()?;
        let mut file_manager = self.file_manager.lock().unwrap();
        let mut page = Page::new(file_manager.block_size);
        dbg!(self.current_block);
        file_manager.read(&self.current_block, &mut page).unwrap();
        Ok(BackwardLogIterator {
            file_manager,
            current_block: self.current_block,
            current_position: self.log_page.get_i32(0) as usize,
            page,
        })
    }

    fn do_flush(&mut self) -> Result<()> {
        self.file_manager
            .lock()
            .unwrap()
            .write(&self.current_block, &self.log_page)?;
        self.last_saved_log_sequence_number = self.latest_log_sequence_number;
        Ok(())
    }
}

pub struct BackwardLogIterator<'a> {
    file_manager: MutexGuard<'a, FileManager>,
    current_position: usize,
    current_block: BlockId,
    page: Page,
}

impl<'a> Iterator for BackwardLogIterator<'a> {
    type Item = LogRecord;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_position == self.file_manager.block_size {
            if self.current_block.block_slot == 0 {
                return None;
            } else {
                let next_block = BlockId::new(
                    self.current_block.file_id,
                    self.current_block.block_slot - 1,
                );
                self.file_manager.read(&next_block, &mut self.page).unwrap();
                self.current_position = self.page.get_i32(0) as usize;
                self.current_block = next_block;
            }
        }
        let log_record = LogRecord::from_bytes(&self.page.byte_buffer[self.current_position..]);
        self.current_position += log_record.to_bytes().len();
        return Some(log_record);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_manager() -> Result<()> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let file_manager = FileManager::new(temp_dir, 1024);
        let mut log_manager = LogManager::new(Arc::new(Mutex::new(file_manager)), "log".into())?;
        log_manager.append_record(&LogRecord::Start(0))?;
        log_manager.flush(1)?;
        Ok(())
    }

    #[test]
    fn test_backward_log_iterator() -> Result<()> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let file_manager = FileManager::new(temp_dir, 50);
        let mut log_manager = LogManager::new(Arc::new(Mutex::new(file_manager)), "log".into())?;
        log_manager.append_record(&LogRecord::Start(0))?;
        log_manager.append_record(&LogRecord::Commit(0))?;
        log_manager.append_record(&LogRecord::Start(1))?;
        log_manager.append_record(&LogRecord::Commit(1))?;
        log_manager.append_record(&LogRecord::Start(2))?;
        log_manager.append_record(&LogRecord::SetI32(2, BlockId::new(0, 0), 0, 0, 0))?;
        let lsn = log_manager.append_record(&LogRecord::Commit(2))?;
        log_manager.flush(lsn)?;

        let mut iter = log_manager.get_backward_iter()?;
        assert_eq!(iter.next(), Some(LogRecord::Commit(2)));
        assert_eq!(
            iter.next(),
            Some(LogRecord::SetI32(2, BlockId::new(0, 0), 0, 0, 0))
        );
        assert_eq!(iter.next(), Some(LogRecord::Start(2)));
        assert_eq!(iter.next(), Some(LogRecord::Commit(1)));
        assert_eq!(iter.next(), Some(LogRecord::Start(1)));
        assert_eq!(iter.next(), Some(LogRecord::Commit(0)));
        assert_eq!(iter.next(), Some(LogRecord::Start(0)));
        assert_eq!(iter.next(), None);
        Ok(())
    }
}
