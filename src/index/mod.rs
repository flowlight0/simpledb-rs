pub mod btree;
pub mod scan;

use btree::btree_index::BTreeIndex;
use enum_dispatch::enum_dispatch;

use crate::{errors::TransactionError, record::field::Value, scan::RecordId};

#[enum_dispatch]
pub enum Index {
    BTree(BTreeIndex),
}

#[enum_dispatch(Index)]
pub trait IndexControl {
    fn before_first(&mut self, search_key: &Value) -> Result<(), TransactionError>;
    fn next(&mut self) -> Result<bool, TransactionError>;
    fn get(&self) -> Result<RecordId, TransactionError>;
    fn insert(&mut self, value: &Value, record_id: &RecordId) -> Result<(), TransactionError>;
    fn delete(&mut self, value: &Value, record_id: &RecordId) -> Result<(), TransactionError>;
}
