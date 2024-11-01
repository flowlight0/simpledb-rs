use std::io::Result;

use crate::file::BlockId;

use super::manager::LogManager;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogRecord {
    Start(usize),
    Commit(usize),
    Checkpoint(usize),
    Rollback(usize),
    SetI32(usize, BlockId, usize, i32, i32),
}

impl LogRecord {
    pub fn to_bytes(&self) -> Vec<u8> {
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
                bytes.extend_from_slice(&offset.to_ne_bytes());
                bytes.extend_from_slice(&old_value.to_ne_bytes());
                bytes.extend_from_slice(&new_value.to_ne_bytes());
                bytes.extend_from_slice(&block_id.to_bytes());
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

    pub fn from_bytes(current_position: &[u8]) -> Self {
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

                let offset = usize::from_ne_bytes([
                    current_position[9],
                    current_position[10],
                    current_position[11],
                    current_position[12],
                    current_position[13],
                    current_position[14],
                    current_position[15],
                    current_position[16],
                ]);
                let old_value = i32::from_ne_bytes([
                    current_position[17],
                    current_position[18],
                    current_position[19],
                    current_position[20],
                ]);
                let new_value = i32::from_ne_bytes([
                    current_position[21],
                    current_position[22],
                    current_position[23],
                    current_position[24],
                ]);
                let (_, block) = BlockId::from_bytes(&current_position[25..]);
                LogRecord::SetI32(transaction_id, block, offset, old_value, new_value)
            }
            _ => panic!("Invalid log record"),
        }
    }
}
