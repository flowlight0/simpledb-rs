use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    errors::TransactionError,
    record::layout::Layout,
    scan::{table_scan::TableScan, Scan},
    tx::transaction::Transaction,
};

use super::table_manager::TableManager;

#[derive(Debug, Clone)]
pub struct StatInfo {
    num_blocks: usize,
    num_records: usize,
}

impl StatInfo {
    pub fn new(num_blocks: usize, num_records: usize) -> Self {
        Self {
            num_blocks,
            num_records,
        }
    }

    // Return the number of blocks in the table
    pub fn get_num_blocks(&self) -> usize {
        self.num_blocks
    }

    // Return the number of records in the table
    pub fn get_num_records(&self) -> usize {
        self.num_records
    }

    #[allow(unused_variables)]
    pub fn get_distinct_values(&self, field_name: &str) -> usize {
        1 + self.num_records / 3 // Fake it for now
    }
}

pub struct StatManager {
    table_manager: Arc<Mutex<TableManager>>,
    table_stats: HashMap<String, StatInfo>,
    num_calls: usize,
}

impl StatManager {
    pub fn new(table_manager: Arc<Mutex<TableManager>>) -> Result<Self, TransactionError> {
        Ok(Self {
            table_manager,
            table_stats: HashMap::new(),
            num_calls: 0,
        })
    }

    pub fn get_stat_info(
        &mut self,
        table_name: &str,
        layout: Arc<Layout>,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<StatInfo, TransactionError> {
        self.num_calls += 1;
        if self.num_calls % 100 == 0 {
            self.refresh_statistics(tx.clone())?;
        }

        if let Some(stat_info) = self.table_stats.get(table_name) {
            return Ok(stat_info.clone());
        } else {
            let stat_info = calculate_table_stat(table_name, layout, tx)?;
            self.table_stats
                .insert(table_name.to_string(), stat_info.clone());
            Ok(stat_info)
        }
    }

    pub(crate) fn refresh_statistics(
        &mut self,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<(), TransactionError> {
        self.table_stats.clear();
        self.num_calls = 0;

        let tcat_layout = Arc::new(
            self.table_manager
                .lock()
                .unwrap()
                .get_layout("tblcat", tx.clone())?
                .unwrap(),
        );
        let mut tcat_scan = TableScan::new(tx.clone(), "tblcat", tcat_layout.clone())?;
        let mut table_names = vec![];
        while tcat_scan.next()? {
            let table_name = tcat_scan.get_string("tblname")?;
            table_names.push(table_name);
        }
        drop(tcat_scan);

        for table_name in table_names {
            let layout = Arc::new(
                self.table_manager
                    .lock()
                    .unwrap()
                    .get_layout(&table_name, tx.clone())?
                    .unwrap(),
            );
            let stat_info = calculate_table_stat(&table_name, layout, tx.clone())?;
            self.table_stats.insert(table_name, stat_info);
        }
        Ok(())
    }
}

fn calculate_table_stat(
    table_name: &str,
    layout: Arc<Layout>,
    tx: Arc<Mutex<Transaction>>,
) -> Result<StatInfo, TransactionError> {
    let mut num_blocks = 0;
    let mut num_records = 0;
    let mut table_scan = TableScan::new(tx, table_name, layout)?;
    while table_scan.next()? {
        num_records += 1;
        num_blocks = table_scan.get_block_slot() + 1;
    }
    Ok(StatInfo::new(num_blocks, num_records))
}
