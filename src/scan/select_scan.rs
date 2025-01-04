use crate::{errors::TransactionError, parser::predicate::Predicate, record::field::Value};

use super::{RecordId, Scan, ScanControl};

pub struct SelectScan {
    base_scan: Box<Scan>,
    predicate: Predicate,
}

impl SelectScan {
    pub fn new(base_scan: Scan, predicate: Predicate) -> Self {
        SelectScan {
            base_scan: Box::new(base_scan),
            predicate,
        }
    }
}

impl ScanControl for SelectScan {
    fn before_first(&mut self) -> Result<(), TransactionError> {
        self.base_scan.before_first()
    }

    fn next(&mut self) -> Result<bool, TransactionError> {
        while self.base_scan.next()? {
            if self.predicate.is_satisfied(&mut self.base_scan)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn get_i32(&mut self, field_name: &str) -> Result<i32, TransactionError> {
        self.base_scan.get_i32(field_name)
    }

    fn get_string(&mut self, field_name: &str) -> Result<String, TransactionError> {
        self.base_scan.get_string(field_name)
    }

    fn get_value(&mut self, field_name: &str) -> Result<Value, TransactionError> {
        self.base_scan.get_value(field_name)
    }

    fn has_field(&self, field_name: &str) -> bool {
        self.base_scan.has_field(field_name)
    }

    fn set_i32(&mut self, field_name: &str, value: i32) -> Result<(), TransactionError> {
        self.base_scan.set_i32(field_name, value)
    }

    fn set_string(&mut self, field_name: &str, value: &str) -> Result<(), TransactionError> {
        self.base_scan.set_string(field_name, value)
    }

    fn delete(&mut self) -> Result<(), TransactionError> {
        self.base_scan.delete()
    }

    fn insert(&mut self) -> Result<(), TransactionError> {
        self.base_scan.insert()
    }

    fn get_record_id(&self) -> RecordId {
        self.base_scan.get_record_id()
    }
}

impl Drop for SelectScan {
    fn drop(&mut self) {}
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::db::SimpleDB;
    use crate::parser::predicate::{Expression, Term};
    use crate::record::layout::Layout;
    use crate::record::schema::Schema;
    use crate::scan::table_scan::TableScan;

    #[test]
    fn test_select_scan() -> Result<(), TransactionError> {
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);
        schema.add_i32_field("C");
        let layout = Arc::new(Layout::new(schema));

        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        let mut table_scan =
            Scan::TableScan(TableScan::new(tx.clone(), "testtable", layout.clone())?);
        table_scan.before_first()?;
        for i in 0..50 {
            table_scan.insert()?;
            table_scan.set_i32("A", i % 3)?;
            table_scan.set_string("B", &(i % 4).to_string())?;
            table_scan.set_i32("C", i)?;
        }

        let mut select_scan = SelectScan::new(
            Scan::from(table_scan),
            Predicate::new(vec![
                Term::Equality(
                    Expression::Field("A".to_string()),
                    Expression::I32Constant(1),
                ),
                Term::Equality(
                    Expression::Field("B".to_string()),
                    Expression::StringConstant("2".to_string()),
                ),
            ]),
        );
        select_scan.before_first()?;
        for i in 0..50 {
            if i % 3 == 1 && i % 4 == 2 {
                assert!(select_scan.next()?);
                assert_eq!(select_scan.get_i32("A")?, 1);
                assert_eq!(select_scan.get_string("B")?, "2");
                assert_eq!(select_scan.get_i32("C")?, i);
                select_scan.set_i32("C", i * 2)?;
            }
        }
        assert!(!select_scan.next()?);
        drop(select_scan);

        let mut table_scan = TableScan::new(tx.clone(), "testtable", layout.clone())?;

        table_scan.before_first()?;
        for i in 0..50 {
            table_scan.next()?;
            if i % 3 == 1 && i % 4 == 2 {
                assert_eq!(table_scan.get_i32("C")?, i * 2);
            } else {
                assert_eq!(table_scan.get_i32("C")?, i);
            }
        }
        drop(table_scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }
}
