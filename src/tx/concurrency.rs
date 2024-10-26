use core::num;
use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex},
    time::Instant,
};

use crate::file::BlockId;

enum Lock {
    None,
    Exclusive,
    Shared(usize),
}

pub struct LockGiveUpError {}

// We assume that each transaction always gets a shared lock before getting an exclusive lock.
// When some transaction tries to get an exclusive lock and there is only one shared lock,
// the shared lock belongs to that transaction.
pub struct LockTable {
    locks: Mutex<HashMap<BlockId, Lock>>,
    condvar: Condvar,
    lock_maxtime: u128,
}

const DEFAULT_LOCK_MAXTIME_IN_MILLIS: u128 = 10_000;

fn has_exclusive_lock(locks: &HashMap<BlockId, Lock>, block: BlockId) -> bool {
    if let Some(lock) = locks.get(&block) {
        match lock {
            Lock::Exclusive => true,
            _ => false,
        }
    } else {
        false
    }
}

fn has_multiple_shared_locks(locks: &HashMap<BlockId, Lock>, block: BlockId) -> bool {
    match locks.get(&block) {
        Some(Lock::Shared(num)) if *num > 1 => true,
        _ => false,
    }
}

fn get_num_shared_locks(locks: &HashMap<BlockId, Lock>, block: BlockId) -> Option<usize> {
    if let Some(lock) = locks.get(&block) {
        match lock {
            Lock::Shared(num) => Some(*num),
            _ => None,
        }
    } else {
        None
    }
}

impl LockTable {
    pub fn new(lock_maxtime: u128) -> Self {
        LockTable {
            locks: Mutex::new(HashMap::new()),
            condvar: Condvar::new(),
            lock_maxtime,
        }
    }

    fn lock_shared(&self, block: BlockId) -> Result<(), LockGiveUpError> {
        let start_time = Instant::now();
        let mut locks = self.locks.lock().unwrap();
        while has_exclusive_lock(&locks, block) {
            if start_time.elapsed().as_millis() > self.lock_maxtime {
                return Err(LockGiveUpError {});
            }
            let duration = std::time::Duration::from_millis(self.lock_maxtime as u64);
            let lock_result = self.condvar.wait_timeout(locks, duration).unwrap();
            locks = lock_result.0;
            if lock_result.1.timed_out() {
                return Err(LockGiveUpError {});
            }
        }
        assert!(!has_exclusive_lock(&locks, block));

        let new_lock = match locks.get(&block) {
            Some(Lock::Shared(num)) => Lock::Shared(num + 1),
            Some(Lock::None) => Lock::Shared(1),
            None => Lock::Shared(1),
            _ => unreachable!(),
        };
        locks.insert(block, new_lock);
        Ok(())
    }

    fn lock_exclusive(&self, block: BlockId) -> Result<(), LockGiveUpError> {
        let start_time = Instant::now();
        let mut locks = self.locks.lock().unwrap();
        while has_multiple_shared_locks(&locks, block) {
            if start_time.elapsed().as_millis() > self.lock_maxtime {
                return Err(LockGiveUpError {});
            }

            let duration = std::time::Duration::from_millis(self.lock_maxtime as u64);
            let lock_result = self.condvar.wait_timeout(locks, duration).unwrap();
            locks = lock_result.0;
            if lock_result.1.timed_out() {
                return Err(LockGiveUpError {});
            }
        }
        assert!(!has_multiple_shared_locks(&locks, block));

        let new_lock = match locks.get(&block) {
            Some(Lock::None) | None => Lock::Exclusive,
            Some(Lock::Shared(num)) if *num == 1 => Lock::Exclusive,
            _ => unreachable!(),
        };
        locks.insert(block, new_lock);
        Ok(())
    }

    fn unlock(&self, block: BlockId) {
        let mut locks = self.locks.lock().unwrap();

        match locks.get(&block) {
            Some(Lock::Exclusive) => {
                locks.remove(&block);
                self.condvar.notify_all();
            }
            Some(Lock::Shared(num)) => {
                if *num == 1 {
                    locks.remove(&block);
                } else {
                    let new_num = *num - 1;
                    locks.insert(block, Lock::Shared(new_num));
                }
                self.condvar.notify_all();
            }
            _ => {}
        }
    }
}

pub struct ConcurrencyManager {
    lock_table: Arc<LockTable>,
    my_locks: HashMap<BlockId, Lock>,
}

// NOTE: Unlike LockTable, ConcurrencyManager is tied with a transaction.
impl ConcurrencyManager {
    pub fn new(lock_table: Arc<LockTable>) -> Self {
        ConcurrencyManager {
            lock_table: lock_table.clone(),
            my_locks: HashMap::new(),
        }
    }

    pub fn lock_shared(&mut self, block: BlockId) -> Result<(), LockGiveUpError> {
        match self.my_locks.get(&block) {
            Some(Lock::Shared(_)) => Ok(()),
            Some(Lock::Exclusive) => Ok(()),
            _ => {
                self.lock_table.lock_shared(block)?;
                self.my_locks.insert(block, Lock::Shared(1));
                Ok(())
            }
        }
    }

    pub fn lock_exclusive(&mut self, block: BlockId) -> Result<(), LockGiveUpError> {
        match self.my_locks.get(&block) {
            Some(Lock::Exclusive) => Ok(()),
            _ => {
                self.lock_shared(block)?;
                self.lock_table.lock_exclusive(block)?;
                self.my_locks.insert(block, Lock::Exclusive);
                Ok(())
            }
        }
    }

    pub fn release(&mut self) {
        for block in self.my_locks.keys() {
            self.lock_table.unlock(*block);
        }
        self.my_locks.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_table() {
        let lock_table = LockTable::new(10);
        assert!(lock_table.lock_shared(BlockId::new(0, 0)).is_ok());
        assert!(lock_table.lock_shared(BlockId::new(0, 0)).is_ok());

        assert!(lock_table.lock_exclusive(BlockId::new(0, 0)).is_err());

        lock_table.unlock(BlockId::new(0, 0));
        assert!(lock_table.lock_exclusive(BlockId::new(0, 0)).is_ok());
    }

    #[test]
    fn test_concurrency_manager() {
        let lock_table = Arc::new(LockTable::new(10));

        // Imitate multiple threads by creating multiple ConcurrencyManager instances
        let mut cm_thread_a = ConcurrencyManager::new(lock_table.clone());
        let mut cm_thread_b = ConcurrencyManager::new(lock_table.clone());

        assert!(cm_thread_a.lock_shared(BlockId::new(0, 0)).is_ok());
        assert!(cm_thread_a.lock_shared(BlockId::new(0, 0)).is_ok());

        // Multiple shared lock on the same block is allowed
        assert!(cm_thread_b.lock_shared(BlockId::new(0, 0)).is_ok());

        // Exclusive lock is not allowed
        assert!(cm_thread_b.lock_exclusive(BlockId::new(0, 0)).is_err());

        // Release the shared lock
        cm_thread_a.release();
        assert!(cm_thread_b.lock_exclusive(BlockId::new(0, 0)).is_ok());
    }
}
