use std::collections::HashMap;

use crate::{
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

    pub fn get_num_blocks(&self) -> usize {
        self.num_blocks
    }

    pub fn get_num_records(&self) -> usize {
        self.num_records
    }

    #[allow(unused_variables)]
    pub fn distinct_values(&self, field_name: &str) -> usize {
        1 + self.num_records / 3 // Fake it for now
    }
}

pub struct StatManager {
    table_manager: TableManager,
    table_stats: HashMap<String, StatInfo>,
    num_calls: usize,
}

impl StatManager {
    pub fn new(is_new: bool, tx: &mut Transaction) -> Result<Self, anyhow::Error> {
        Ok(Self {
            table_manager: TableManager::new(is_new, tx)?,
            table_stats: HashMap::new(),
            num_calls: 0,
        })
    }

    pub fn get_stat_info(
        &mut self,
        table_name: &str,
        layout: &Layout,
        tx: &mut Transaction,
    ) -> Result<StatInfo, anyhow::Error> {
        self.num_calls += 1;
        if self.num_calls % 100 == 0 {
            self.refresh_statistics(tx)?;
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

    fn refresh_statistics(&mut self, tx: &mut Transaction) -> Result<(), anyhow::Error> {
        self.table_stats.clear();
        self.num_calls = 0;

        let tcat_layout = self.table_manager.get_layout("tblcat", tx)?.unwrap();
        let mut tcat_scan = TableScan::new(tx, "tblcat", &tcat_layout)?;
        let mut table_names = vec![];
        while tcat_scan.next()? {
            let table_name = tcat_scan.get_string("tblname")?;
            table_names.push(table_name);
        }
        drop(tcat_scan);

        for table_name in table_names {
            let layout = self.table_manager.get_layout(&table_name, tx)?.unwrap();
            let stat_info = calculate_table_stat(&table_name, &layout, tx)?;
            self.table_stats.insert(table_name, stat_info);
        }
        Ok(())
    }
}

fn calculate_table_stat(
    table_name: &str,
    layout: &Layout,
    tx: &mut Transaction,
) -> Result<StatInfo, anyhow::Error> {
    let mut num_blocks = 0;
    let mut num_records = 0;
    let mut table_scan = TableScan::new(tx, table_name, layout)?;
    while table_scan.next()? {
        num_records += 1;
        num_blocks = table_scan.get_block_number() + 1;
    }
    Ok(StatInfo::new(num_blocks, num_records))
}