pub mod btree;

use btree::BTreeIndex;
use enum_dispatch::enum_dispatch;

use crate::{record::field::Value, scan::RecordId};

#[enum_dispatch]
pub enum Index {
    BTree(BTreeIndex),
}

#[enum_dispatch(Index)]
pub trait IndexControl {
    fn before_first(&mut self, search_key: Value) -> Result<(), anyhow::Error>;
    fn next(&mut self) -> Result<bool, anyhow::Error>;
    fn get(&self) -> Result<RecordId, anyhow::Error>;
    fn insert(&mut self, value: Value, record_id: RecordId) -> Result<(), anyhow::Error>;
    fn delete(&mut self, value: Value, record_id: RecordId) -> Result<(), anyhow::Error>;
    fn close(&mut self) -> Result<(), anyhow::Error>;
}
