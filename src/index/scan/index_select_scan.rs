use crate::{
    errors::TransactionError,
    index::{Index, IndexControl},
    record::field::Value,
    scan::{table_scan::TableScan, RecordPointer, ScanControl},
};

pub struct IndexSelectScan {
    table_scan: TableScan,
    index: Index,
    value: Value,
}

impl IndexSelectScan {
    pub fn new(
        table_scan: TableScan,
        index: Index,
        value: Value,
    ) -> Result<Self, TransactionError> {
        let mut scan = IndexSelectScan {
            table_scan,
            index,
            value,
        };
        scan.before_first()?;
        Ok(scan)
    }
}

impl ScanControl for IndexSelectScan {
    fn before_first(&mut self) -> Result<(), TransactionError> {
        self.index.before_first(&self.value)
    }

    fn next(&mut self) -> Result<bool, TransactionError> {
        let has_next = self.index.next()?;
        if has_next {
            let record_pointer = RecordPointer::from(self.index.get()?);
            self.table_scan.move_to_record_pointer(&record_pointer)?;
        }
        Ok(has_next)
    }

    fn get_i32(&mut self, field_name: &str) -> Result<Option<i32>, TransactionError> {
        self.table_scan.get_i32(field_name)
    }

    fn get_string(&mut self, field_name: &str) -> Result<Option<String>, TransactionError> {
        self.table_scan.get_string(field_name)
    }

    fn get_value(&mut self, field_name: &str) -> Result<Value, TransactionError> {
        self.table_scan.get_value(field_name)
    }

    fn has_field(&self, field_name: &str) -> bool {
        self.table_scan.has_field(field_name)
    }

    fn get_record_pointer(&self) -> RecordPointer {
        self.table_scan.get_record_pointer()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        db::SimpleDB,
        record::{layout::Layout, schema::Schema},
        scan::RecordId,
    };

    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_index_select_scan() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 1024;
        let num_buffers = 256;

        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);
        let layout = Arc::new(Layout::new(schema));

        let test_table_name = "test_table";
        let test_index_name = "test_index";
        let mut table_scan = TableScan::new(tx.clone(), test_table_name, layout.clone())?;
        db.metadata_manager.lock().unwrap().create_table(
            test_table_name,
            &layout.schema,
            tx.clone(),
        )?;
        db.metadata_manager.lock().unwrap().create_index(
            test_index_name,
            test_table_name,
            "B",
            tx.clone(),
        )?;
        let mut index = db
            .metadata_manager
            .lock()
            .unwrap()
            .get_index_info(test_table_name, tx.clone())?
            .get("B")
            .unwrap()
            .open()?;

        let mut expected_values = vec![];
        table_scan.before_first()?;
        for i in 0..50 {
            table_scan.insert()?;
            table_scan.set_i32("A", i)?;

            let b_value = (i % 4).to_string();
            table_scan.set_string("B", &b_value)?;
            index.insert(
                &Value::String(b_value.clone()),
                &RecordId::from(table_scan.get_record_pointer()),
            )?;
            if b_value == "2".to_owned() {
                expected_values.push(i);
            }
        }

        let mut select_scan =
            IndexSelectScan::new(table_scan, index, Value::String("2".to_owned()))?;
        select_scan.before_first()?;

        let mut actual_values = vec![];
        while select_scan.next()? {
            actual_values.push(select_scan.get_i32("A")?.unwrap());
        }
        actual_values.sort();

        assert_eq!(expected_values, actual_values);
        assert!(!select_scan.next()?);
        drop(select_scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }
}
