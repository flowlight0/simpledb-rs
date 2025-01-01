use crate::record::field::Value;

mod btree_directory;
pub mod btree_index;
mod btree_leaf;
mod btree_page;

pub(crate) struct DirectoryEntry {
    pub(crate) data_value: Value,
    pub(crate) block_slot: usize,
}
