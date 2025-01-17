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
    SetString(usize, BlockId, usize, String, String),
}

fn from_ne_bytes_to_usize(bytes: &[u8]) -> usize {
    usize::from_ne_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

fn from_ne_bytes_to_i32(bytes: &[u8]) -> i32 {
    i32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
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
            LogRecord::SetString(transaction_id, block_id, offset, old_string, new_string) => {
                let mut bytes = Vec::new();
                bytes.push('T' as u8);
                bytes.extend_from_slice(&transaction_id.to_ne_bytes());
                bytes.extend_from_slice(&offset.to_ne_bytes());
                bytes.extend_from_slice(&old_string.len().to_ne_bytes());
                bytes.extend_from_slice(old_string.as_bytes());
                bytes.extend_from_slice(&new_string.len().to_ne_bytes());
                bytes.extend_from_slice(new_string.as_bytes());
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
            LogRecord::SetString(transaction_id, _, _, _, _) => *transaction_id,
        }
    }

    pub fn from_bytes(current_position: &[u8]) -> Self {
        match current_position[0] as char {
            'S' => {
                let transaction_id = from_ne_bytes_to_usize(&current_position[1..9]);
                LogRecord::Start(transaction_id)
            }
            'C' => {
                let transaction_id = from_ne_bytes_to_usize(&current_position[1..9]);
                LogRecord::Commit(transaction_id)
            }
            'K' => {
                let transaction_id = from_ne_bytes_to_usize(&current_position[1..9]);
                LogRecord::Checkpoint(transaction_id)
            }
            'R' => {
                let transaction_id = from_ne_bytes_to_usize(&current_position[1..9]);
                LogRecord::Rollback(transaction_id)
            }
            'I' => {
                let transaction_id = from_ne_bytes_to_usize(&current_position[1..9]);
                let offset = from_ne_bytes_to_usize(&current_position[9..17]);
                let old_value = from_ne_bytes_to_i32(&current_position[17..21]);
                let new_value = from_ne_bytes_to_i32(&current_position[21..25]);
                let (_, block) = BlockId::from_bytes(&current_position[25..]);
                LogRecord::SetI32(transaction_id, block, offset, old_value, new_value)
            }
            'T' => {
                let transaction_id = from_ne_bytes_to_usize(&current_position[1..9]);
                let offset = from_ne_bytes_to_usize(&current_position[9..17]);
                let old_string_length = from_ne_bytes_to_usize(&current_position[17..25]);
                let old_string =
                    String::from_utf8(current_position[25..25 + old_string_length].to_vec())
                        .unwrap();
                let new_string_length = from_ne_bytes_to_usize(
                    &current_position[25 + old_string_length..33 + old_string_length],
                );
                let new_string = String::from_utf8(
                    current_position
                        [33 + old_string_length..33 + old_string_length + new_string_length]
                        .to_vec(),
                )
                .unwrap();
                let (_, block) = BlockId::from_bytes(
                    &current_position[33 + old_string_length + new_string_length..],
                );
                LogRecord::SetString(transaction_id, block, offset, old_string, new_string)
            }
            _ => panic!("Invalid log record"),
        }
    }
}
