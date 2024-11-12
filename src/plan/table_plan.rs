use crate::{
    metadata::{stat_manager::StatInfo, MetadataManager},
    record::{layout::Layout, schema::Schema},
    scan::{table_scan::TableScan, Scan},
    tx::transaction::Transaction,
};

use super::Plan;

pub struct TablePlan {
    table_name: String,
    layout: Layout,
    stat_info: StatInfo,
}

impl TablePlan {
    pub fn new(
        tx: &mut Transaction,
        table_name: &str,
        metadata_manager: &mut MetadataManager,
    ) -> Result<Self, anyhow::Error> {
        let layout = metadata_manager.get_layout(table_name, tx)?.unwrap();
        let stat_info = metadata_manager.get_stat_info(table_name, &layout, tx)?;
        Ok(TablePlan {
            table_name: table_name.to_string(),
            layout,
            stat_info,
        })
    }
}

impl Plan for TablePlan {
    fn get_num_accessed_blocks(&self) -> usize {
        self.stat_info.get_num_blocks()
    }

    fn get_num_output_records(&self) -> usize {
        self.stat_info.get_num_records()
    }

    fn num_distinct_values(&self, field_name: &str) -> usize {
        self.stat_info.get_distinct_values(field_name)
    }

    fn schema(&self) -> Schema {
        self.layout.schema.clone()
    }

    fn open<'a>(
        &'a mut self,
        tx: &'a mut Transaction,
    ) -> Result<Box<dyn Scan + 'a>, anyhow::Error> {
        let table_scan = TableScan::new(tx, &self.table_name, &self.layout)?;
        Ok(Box::new(table_scan))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::SimpleDB;
    use crate::record::layout::Layout;
    use crate::record::schema::Schema;

    #[test]
    fn test_table_plan() -> Result<(), anyhow::Error> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let num_buffers = 3;
        let mut db = SimpleDB::new(temp_dir, block_size, num_buffers)?;

        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);
        let layout = Layout::new(schema);

        let table_name = "testtable";
        let mut tx = db.new_transaction()?;

        db.metadata_manager
            .create_table(table_name, &layout.schema, &mut tx)?;
        let mut table_scan = TableScan::new(&mut tx, table_name, &layout)?;
        table_scan.before_first()?;
        for i in 0..200 {
            table_scan.insert()?;
            table_scan.set_i32("A", i)?;
            table_scan.set_string("B", &i.to_string())?;
        }
        drop(table_scan);
        tx.commit()?;

        let mut tx = db.new_transaction()?;
        db.metadata_manager
            .stat_manager
            .refresh_statistics(&mut tx)?;
        let table_plan = TablePlan::new(&mut tx, table_name, &mut db.metadata_manager)?;
        assert!(table_plan.get_num_accessed_blocks() > 0);
        assert_eq!(table_plan.get_num_output_records(), 200);
        assert!(table_plan.num_distinct_values("A") > 0);
        Ok(())
    }
}
