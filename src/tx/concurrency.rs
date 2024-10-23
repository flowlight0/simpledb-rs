use std::{
    collections::HashMap,
    sync::{Condvar, Mutex},
    time::Instant,
};

use crate::file::BlockId;

enum Lock {
    None,
    Exclusive,
    Shared(usize),
}

struct LockTable {
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

fn has_any_lock(locks: &HashMap<BlockId, Lock>, block: BlockId) -> bool {
    if let Some(lock) = locks.get(&block) {
        match lock {
            Lock::None => false,
            _ => true,
        }
    } else {
        false
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
    fn new(lock_maxtime: u128) -> Self {
        LockTable {
            locks: Mutex::new(HashMap::new()),
            condvar: Condvar::new(),
            lock_maxtime,
        }
    }

    fn lock_shared(&mut self, block: BlockId) -> bool {
        let start_time = Instant::now();
        let mut locks = self.locks.lock().unwrap();
        while has_exclusive_lock(&locks, block) {
            if start_time.elapsed().as_millis() > self.lock_maxtime {
                return false;
            }
            let duration = std::time::Duration::from_millis(self.lock_maxtime as u64);
            let lock_result = self.condvar.wait_timeout(locks, duration).unwrap();
            locks = lock_result.0;
            if lock_result.1.timed_out() {
                return false;
            }
        }

        if has_exclusive_lock(&locks, block) {
            return false;
        }

        let new_lock = match locks.get(&block) {
            Some(Lock::Shared(num)) => Lock::Shared(num + 1),
            Some(Lock::None) => Lock::Shared(1),
            None => Lock::Shared(1),
            _ => unreachable!(),
        };
        locks.insert(block, new_lock);
        true
    }

    fn lock_exclusive(&mut self, block: BlockId) -> bool {
        let start_time = Instant::now();
        let mut locks = self.locks.lock().unwrap();
        while has_any_lock(&locks, block) {
            if start_time.elapsed().as_millis() > self.lock_maxtime {
                return false;
            }

            let duration = std::time::Duration::from_millis(self.lock_maxtime as u64);
            let lock_result = self.condvar.wait_timeout(locks, duration).unwrap();
            locks = lock_result.0;
            if lock_result.1.timed_out() {
                return false;
            }
        }

        if has_any_lock(&locks, block) {
            return false;
        }

        let new_lock = match locks.get(&block) {
            Some(Lock::None) => Lock::Exclusive,
            None => Lock::Exclusive,
            _ => unreachable!(),
        };
        locks.insert(block, new_lock);
        true
    }

    fn unlock(&mut self, block: BlockId) {
        let mut locks = self.locks.lock().unwrap();
        if let Some(lock) = locks.get(&block) {
            match lock {
                Lock::Exclusive => {
                    locks.remove(&block);
                    self.condvar.notify_all();
                }
                Lock::Shared(num) => {
                    if *num == 1 {
                        locks.remove(&block);
                    } else {
                        let new_num = *num - 1;
                        locks.insert(block, Lock::Shared(new_num));
                    }
                    self.condvar.notify_all();
                }
                Lock::None => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_table() {
        let mut lock_table = LockTable::new(10);
        assert!(lock_table.lock_shared(BlockId::new(0, 0)));
        assert!(lock_table.lock_shared(BlockId::new(0, 0)));

        assert!(!lock_table.lock_exclusive(BlockId::new(0, 0)));

        lock_table.unlock(BlockId::new(0, 0));
        assert!(!lock_table.lock_exclusive(BlockId::new(0, 0)));

        lock_table.unlock(BlockId::new(0, 0));
        assert!(lock_table.lock_exclusive(BlockId::new(0, 0)));
    }
}
