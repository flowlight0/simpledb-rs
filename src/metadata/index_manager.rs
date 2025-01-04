use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    errors::TransactionError,
    index::{btree::btree_index::BTreeIndex, Index},
    record::{field::Spec, layout::Layout, schema::Schema},
    scan::{table_scan::TableScan, ScanControl},
    tx::transaction::Transaction,
};

use super::{
    stat_manager::{StatInfo, StatManager},
    table_manager::TableManager,
};

const INDEX_TABLE_NAME: &str = "idxcat";
const FIELD_NAME_COLUMN: &str = "field_name";
const INDEX_NAME_COLUMN: &str = "index_name";
const TABLE_NAME_COLUMN: &str = "table_name";
pub const INDEX_VALUE_COLUMN: &str = "data_value";
pub const INDEX_BLOCK_SLOT_COLUMN: &str = "block_slot";
pub const INDEX_RECORD_SLOT_COLUMN: &str = "record_slot";

const MAX_LENGTH: usize = 255;

pub struct IndexInfo {
    index_name: String,
    field_name: String,
    index_layout: Layout,
    // schema: Schema,
    tx: Arc<Mutex<Transaction>>,
    stat_info: StatInfo,
}

fn create_index_layout(table_schema: &Schema, field_name: &str) -> Layout {
    let mut schema = Schema::new();
    schema.add_i32_field(INDEX_BLOCK_SLOT_COLUMN);
    schema.add_i32_field(INDEX_RECORD_SLOT_COLUMN);
    let field_spec = table_schema.get_field_spec(field_name);
    match field_spec {
        Spec::I32 => schema.add_i32_field(INDEX_VALUE_COLUMN),
        Spec::VarChar(len) => schema.add_string_field(INDEX_VALUE_COLUMN, len),
    }
    Layout::new(schema)
}

impl IndexInfo {
    fn new(
        index_name: String,
        field_name: String,
        schema: Schema,
        tx: Arc<Mutex<Transaction>>,
        stat_info: StatInfo,
    ) -> Self {
        let index_layout = create_index_layout(&schema, &field_name);
        IndexInfo {
            index_name,
            field_name,
            index_layout,
            // schema,
            tx,
            stat_info,
        }
    }

    pub fn open(&self) -> Result<Index, TransactionError> {
        Ok(Index::BTree(BTreeIndex::new(
            self.tx.clone(),
            self.index_name.clone(),
            self.index_layout.clone(),
        )?))
    }

    pub fn get_num_accessed_blocks(&self) -> usize {
        let block_size = self.tx.lock().unwrap().get_block_size();
        let records_per_block = block_size / self.index_layout.slot_size;
        let num_blocks = self.stat_info.get_num_records() / records_per_block;
        BTreeIndex::get_search_cost(num_blocks, records_per_block)
    }

    pub fn get_num_records(&self) -> usize {
        self.stat_info.get_num_records() / self.stat_info.get_distinct_values(&self.field_name)
    }

    pub fn get_distinct_values(&self, field_name: &str) -> usize {
        if field_name == self.field_name {
            1
        } else {
            self.stat_info.get_distinct_values(field_name)
        }
    }
}

pub struct IndexManager {
    table_manager: Arc<Mutex<TableManager>>,
    stat_manager: Arc<Mutex<StatManager>>,
    tx: Arc<Mutex<Transaction>>,
    layout: Arc<Layout>,
}

impl IndexManager {
    pub fn new(
        is_new: bool,
        table_manager: Arc<Mutex<TableManager>>,
        stat_manager: Arc<Mutex<StatManager>>,
        tx: Arc<Mutex<Transaction>>,
    ) -> Self {
        if is_new {
            let mut schema = Schema::new();
            schema.add_string_field(INDEX_NAME_COLUMN, MAX_LENGTH);
            schema.add_string_field(TABLE_NAME_COLUMN, MAX_LENGTH);
            schema.add_string_field(FIELD_NAME_COLUMN, MAX_LENGTH);
            let table_manager = table_manager.lock().unwrap();
            table_manager
                .create_table(INDEX_TABLE_NAME, &schema, tx.clone())
                .unwrap();
        }
        let layout = table_manager
            .lock()
            .unwrap()
            .get_layout(INDEX_TABLE_NAME, tx.clone())
            .unwrap()
            .unwrap();
        let layout = Arc::new(layout);

        IndexManager {
            table_manager,
            stat_manager,
            tx,
            layout,
        }
    }

    pub fn create_index(
        &self,
        index_name: &str,
        table_name: &str,
        field_name: &str,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<(), TransactionError> {
        let mut scan = TableScan::new(tx, INDEX_TABLE_NAME, self.layout.clone())?;
        scan.insert()?;
        scan.set_string(INDEX_NAME_COLUMN, index_name)?;
        scan.set_string(TABLE_NAME_COLUMN, table_name)?;
        scan.set_string(FIELD_NAME_COLUMN, field_name)
    }

    pub fn get_index_info(
        &self,
        table_name: &str,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<HashMap<String, IndexInfo>, TransactionError> {
        let mut result = HashMap::new();
        let mut scan = TableScan::new(tx.clone(), INDEX_TABLE_NAME, self.layout.clone())?;

        while scan.next()? {
            if scan.get_string(TABLE_NAME_COLUMN)? != table_name {
                continue;
            }

            let index_name = scan.get_string(INDEX_NAME_COLUMN)?;
            let field_name = scan.get_string(FIELD_NAME_COLUMN)?;

            let layout = self
                .table_manager
                .lock()
                .unwrap()
                .get_layout(table_name, tx.clone())?
                .expect("table not found");
            let stat_info = self.stat_manager.lock().unwrap().get_stat_info(
                table_name,
                Arc::new(layout.clone()),
                tx.clone(),
            )?;
            let index_info = IndexInfo::new(
                index_name.clone(),
                field_name,
                layout.schema.clone(),
                tx.clone(),
                stat_info,
            );
            result.insert(index_name, index_info);
        }
        Ok(result)
    }
}
