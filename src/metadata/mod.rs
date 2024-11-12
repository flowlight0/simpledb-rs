use stat_manager::{StatInfo, StatManager};
use table_manager::TableManager;

use crate::{
    record::{layout::Layout, schema::Schema},
    tx::transaction::Transaction,
};

pub mod stat_manager;
pub mod table_manager;

pub struct MetadataManager {
    table_manager: TableManager,
    stat_manager: StatManager,
}

impl MetadataManager {
    pub fn new(is_new: bool, tx: &mut Transaction) -> Result<Self, anyhow::Error> {
        Ok(Self {
            table_manager: TableManager::new(is_new, tx)?,
            stat_manager: StatManager::new(is_new, tx)?,
        })
    }

    pub fn create_table(
        &mut self,
        table_name: &str,
        schema: &Schema,
        tx: &mut Transaction,
    ) -> Result<(), anyhow::Error> {
        self.table_manager.create_table(table_name, schema, tx)
    }

    pub fn get_layout(
        &self,
        table_name: &str,
        tx: &mut Transaction,
    ) -> Result<Option<Layout>, anyhow::Error> {
        self.table_manager.get_layout(table_name, tx)
    }

    pub fn get_stat_info(
        &mut self,
        table_name: &str,
        layout: &Layout,
        tx: &mut Transaction,
    ) -> Result<StatInfo, anyhow::Error> {
        self.stat_manager.get_stat_info(table_name, layout, tx)
    }
}

#[cfg(test)]
mod tests {
    use crate::db::SimpleDB;
    use crate::metadata::MetadataManager;

    use crate::record::schema::Schema;
    use crate::scan::table_scan::TableScan;
    use crate::scan::Scan;

    #[test]
    fn test_metadata_manager() -> Result<(), anyhow::Error> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let num_buffers = 3;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;

        let mut tx = db.new_transaction()?;
        let mut metadata_manager = MetadataManager::new(true, &mut tx)?;

        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);

        metadata_manager.create_table("testtable", &schema, &mut tx)?;
        let layout = metadata_manager.get_layout("testtable", &mut tx)?.unwrap();
        assert_eq!(&layout.schema, &schema);

        let mut table_scan = TableScan::new(&mut tx, "testtable", &layout)?;
        for i in 0..50 {
            table_scan.insert()?;
            table_scan.set_i32("A", i)?;
            table_scan.set_string("B", &i.to_string())?;
        }

        drop(table_scan);

        let stat_info = metadata_manager.get_stat_info("testtable", &layout, &mut tx)?;
        assert_eq!(stat_info.get_num_records(), 50);
        // It's obvious that the number of blocks is greater than 1
        assert!(stat_info.get_num_blocks() > 1);
        Ok(())
    }
}
