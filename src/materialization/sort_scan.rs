use std::{cell::RefCell, cmp::Ordering, sync::Arc};

use crate::{
    errors::TransactionError,
    record::field::Value,
    scan::{RecordPointer, Scan, ScanControl},
};

use super::{record_comparator::RecordComparator, temp_table::TempTable};

pub struct SortScan {
    scans: Vec<RefCell<Scan>>,
    has_nexts: Vec<bool>,
    comparator: Arc<RecordComparator>,
    current_scan_index: Option<usize>,
    saved_position: Vec<RecordPointer>,
}

impl SortScan {
    pub fn new(
        mut runs: Vec<TempTable>,
        comparator: Arc<RecordComparator>,
    ) -> Result<Self, TransactionError> {
        if runs.len() > 2 || runs.len() == 0 {
            panic!("The number of runs must be 1 or 2");
        }

        if runs.len() == 1 {
            // Add dummy empty table
            runs.push(TempTable::new(
                runs[0].get_tx(),
                &runs[0].get_layout().schema,
            ));
        }

        let mut scans = Vec::new();
        let mut has_nexts = Vec::new();
        for run in runs {
            let mut scan = run.open()?;
            let has_next = scan.next().unwrap();
            scans.push(RefCell::new(Scan::from(scan)));
            has_nexts.push(has_next);
        }

        Ok(SortScan {
            scans,
            has_nexts,
            comparator,
            current_scan_index: None,
            saved_position: vec![],
        })
    }

    pub(crate) fn restore_position(&mut self) -> Result<(), TransactionError> {
        for (i, scan) in self.scans.iter().enumerate() {
            scan.borrow_mut()
                .move_to_record_pointer(&self.saved_position[i])?;
        }
        Ok(())
    }

    pub(crate) fn save_position(&mut self) {
        self.saved_position = self
            .scans
            .iter()
            .map(|scan| scan.borrow().get_record_pointer())
            .collect();
    }
}

impl ScanControl for SortScan {
    fn before_first(&mut self) -> Result<(), TransactionError> {
        self.current_scan_index = None;
        for i in 0..self.scans.len() {
            self.scans[i].borrow_mut().before_first()?;
            self.has_nexts[i] = self.scans[i].borrow_mut().next()?;
        }
        Ok(())
    }

    fn next(&mut self) -> Result<bool, TransactionError> {
        if let Some(index) = self.current_scan_index {
            self.has_nexts[index] = self.scans[index].borrow_mut().next()?;
        }

        if self.has_nexts.iter().all(|&x| !x) {
            return Ok(false);
        } else if self.has_nexts.iter().all(|&x| x) {
            let cmp = self.comparator.compare(
                &mut *self.scans[0].borrow_mut(),
                &mut *self.scans[1].borrow_mut(),
            );
            if cmp == Ordering::Less {
                self.current_scan_index = Some(0);
            } else {
                self.current_scan_index = Some(1);
            }
        } else if self.has_nexts[0] {
            // only 0 has next
            self.current_scan_index = Some(0);
        } else {
            // only 1 has next
            self.current_scan_index = Some(1);
        }
        Ok(true)
    }

    fn get_i32(&mut self, field_name: &str) -> Result<i32, TransactionError> {
        self.scans[self.current_scan_index.unwrap()]
            .borrow_mut()
            .get_i32(field_name)
    }

    fn get_string(&mut self, field_name: &str) -> Result<String, TransactionError> {
        self.scans[self.current_scan_index.unwrap()]
            .borrow_mut()
            .get_string(field_name)
    }

    fn get_value(&mut self, field_name: &str) -> Result<Value, TransactionError> {
        self.scans[self.current_scan_index.unwrap()]
            .borrow_mut()
            .get_value(field_name)
    }

    fn has_field(&self, field_name: &str) -> bool {
        self.scans[self.current_scan_index.unwrap()]
            .borrow()
            .has_field(field_name)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::{db::SimpleDB, record::schema::Schema};

    use super::*;

    #[test]
    fn test_sort_scan() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 1024;
        let num_buffers = 100;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);

        let temp_table1 = TempTable::new(tx.clone(), &schema);
        let temp_table2 = TempTable::new(tx.clone(), &schema);

        {
            let mut scan = temp_table1.open()?;
            scan.before_first()?;
            for i in 0..10 {
                let v = i * 2 + 0;
                scan.insert()?;
                scan.set_i32("A", v)?;
                scan.set_string("B", &v.to_string())?;
            }
        }
        {
            let mut scan = temp_table2.open()?;
            scan.before_first()?;
            for i in 0..10 {
                let v = i * 2 + 1;
                scan.insert()?;
                scan.set_i32("A", v)?;
                scan.set_string("B", &v.to_string())?;
            }
        }

        let sort_scan = SortScan::new(
            vec![temp_table1, temp_table2],
            Arc::new(RecordComparator::new(&vec!["A".to_owned()])),
        )?;
        let mut scan = Scan::from(sort_scan);
        scan.before_first()?;
        for i in 0..20 {
            assert_eq!(scan.next()?, true);
            assert_eq!(scan.get_i32("A")?, i);
            assert_eq!(scan.get_string("B")?, i.to_string());
        }
        assert_eq!(scan.next()?, false);

        Ok(())
    }
}
