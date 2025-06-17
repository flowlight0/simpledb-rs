use crate::{
    errors::TransactionError,
    index::{Index, IndexControl},
    scan::{table_scan::TableScan, RecordPointer, Scan, ScanControl},
};

pub struct IndexJoinScan {
    lhs: Box<Scan>,
    rhs_index: Index,
    join_field: String,
    rhs: TableScan,
    is_empty: bool,
    results: Vec<(RecordPointer, RecordPointer)>,
    current_index: Option<usize>,
}

impl IndexJoinScan {
    pub fn new(
        lhs: Scan,
        rhs_index: Index,
        join_field: String,
        rhs: TableScan,
    ) -> Result<Self, TransactionError> {
        let mut scan = IndexJoinScan {
            lhs: Box::new(lhs),
            rhs_index,
            join_field,
            rhs,
            is_empty: false,
            results: vec![],
            current_index: None,
        };
        scan.load_results()?;
        scan.before_first()?;
        Ok(scan)
    }

    fn reset_index(&mut self) -> Result<(), TransactionError> {
        let search_key = self.lhs.get_value(&self.join_field)?;
        self.rhs_index.before_first(&search_key)
    }

    fn reset_scans(&mut self) -> Result<(), TransactionError> {
        self.lhs.before_first()?;
        self.is_empty = !self.lhs.next()?;
        self.reset_index()?;
        Ok(())
    }

    fn load_results(&mut self) -> Result<(), TransactionError> {
        self.results.clear();
        self.reset_scans()?;
        while self.next_internal()? {
            self.results.push((
                self.lhs.get_record_pointer(),
                self.rhs.get_record_pointer(),
            ));
        }
        self.reset_scans()?;
        Ok(())
    }

    fn next_internal(&mut self) -> Result<bool, TransactionError> {
        if self.is_empty {
            return Ok(false);
        }
        loop {
            if self.rhs_index.next()? {
                self.rhs
                    .move_to_record_pointer(&RecordPointer::from(self.rhs_index.get()?))?;
                return Ok(true);
            }
            if !self.lhs.next()? {
                return Ok(false);
            }
            self.reset_index()?;
        }
    }
}

impl ScanControl for IndexJoinScan {
    fn before_first(&mut self) -> Result<(), TransactionError> {
        self.reset_scans()?;
        self.current_index = None;
        Ok(())
    }

    fn next(&mut self) -> Result<bool, TransactionError> {
        let next_index = match self.current_index {
            None => 0,
            Some(i) => i + 1,
        };
        if next_index >= self.results.len() {
            self.current_index = Some(self.results.len());
            return Ok(false);
        }
        let (lhs_rp, rhs_rp) = self.results[next_index];
        self.lhs.move_to_record_pointer(&lhs_rp)?;
        self.rhs.move_to_record_pointer(&rhs_rp)?;
        self.current_index = Some(next_index);
        Ok(true)
    }

    fn after_last(&mut self) -> Result<(), TransactionError> {
        self.current_index = Some(self.results.len());
        Ok(())
    }

    fn previous(&mut self) -> Result<bool, TransactionError> {
        let prev_index = match self.current_index {
            None => return Ok(false),
            Some(0) => return Ok(false),
            Some(i) if i > self.results.len() => {
                if self.results.is_empty() {
                    return Ok(false);
                }
                self.results.len() - 1
            }
            Some(i) => i - 1,
        };
        let (lhs_rp, rhs_rp) = self.results[prev_index];
        self.lhs.move_to_record_pointer(&lhs_rp)?;
        self.rhs.move_to_record_pointer(&rhs_rp)?;
        self.current_index = Some(prev_index);
        Ok(true)
    }

    fn get_i32(&mut self, field_name: &str) -> Result<i32, TransactionError> {
        if self.rhs.has_field(field_name) {
            self.rhs.get_i32(field_name)
        } else {
            self.lhs.get_i32(field_name)
        }
    }

    fn get_string(&mut self, field_name: &str) -> Result<String, TransactionError> {
        if self.rhs.has_field(field_name) {
            self.rhs.get_string(field_name)
        } else {
            self.lhs.get_string(field_name)
        }
    }

    fn get_value(
        &mut self,
        field_name: &str,
    ) -> Result<crate::record::field::Value, TransactionError> {
        if self.rhs.has_field(field_name) {
            self.rhs.get_value(field_name)
        } else {
            self.lhs.get_value(field_name)
        }
    }

    fn has_field(&self, field_name: &str) -> bool {
        self.rhs.has_field(field_name) || self.lhs.has_field(field_name)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{
        db::SimpleDB,
        record::{field::Value, layout::Layout, schema::Schema},
        scan::RecordId,
    };

    use super::*;

    #[test]
    fn test_index_join_scan() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 1024;
        let num_buffers = 256;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;

        let mut schema1 = Schema::new();
        schema1.add_i32_field("A");
        schema1.add_string_field("B", 20);
        let layout1 = Arc::new(Layout::new(schema1));

        let mut schema2 = Schema::new();
        schema2.add_string_field("B", 20);
        schema2.add_i32_field("C");
        let layout2 = Arc::new(Layout::new(schema2));

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        let table1 = "test_table1";
        let table2 = "test_table2";
        let index2 = "test_index2";
        db.metadata_manager
            .lock()
            .unwrap()
            .create_table(table1, &layout1.schema, tx.clone())?;
        db.metadata_manager
            .lock()
            .unwrap()
            .create_table(table2, &layout2.schema, tx.clone())?;

        db.metadata_manager
            .lock()
            .unwrap()
            .create_index(index2, table2, "B", tx.clone())?;

        let mut index = db
            .metadata_manager
            .lock()
            .unwrap()
            .get_index_info(table2, tx.clone())?
            .get("B")
            .unwrap()
            .open()?;

        {
            let mut scan1 = TableScan::new(tx.clone(), table1, layout1.clone())?;
            scan1.before_first()?;
            for i in 0..100 {
                if i % 3 == 0 {
                    scan1.insert()?;
                    scan1.set_i32("A", i)?;
                    scan1.set_string("B", &i.to_string())?;
                }
            }
        }

        {
            let mut scan2 = TableScan::new(tx.clone(), &table2, layout2.clone())?;
            scan2.before_first()?;
            for i in 0..100 {
                if i % 4 == 0 {
                    scan2.insert()?;
                    scan2.set_string("B", &i.to_string())?;
                    scan2.set_i32("C", i * 10)?;
                    index.insert(
                        &Value::String(i.to_string()),
                        &RecordId::from(scan2.get_record_pointer()),
                    )?;
                }
            }
        }

        let mut index_join_scan = IndexJoinScan::new(
            Scan::from(TableScan::new(tx.clone(), table1, layout1.clone())?),
            index,
            "B".to_string(),
            TableScan::new(tx.clone(), table2, layout2.clone())?,
        )?;

        let expected_values: Vec<(i32, i32)> = (0..100)
            .filter(|i| i % 12 == 0)
            .map(|i| (i, i * 10))
            .collect();
        let mut actual_values = vec![];
        while index_join_scan.next()? {
            actual_values.push((index_join_scan.get_i32("A")?, index_join_scan.get_i32("C")?));
        }
        actual_values.sort();
        assert_eq!(expected_values, actual_values);

        drop(index_join_scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }
}
