use crate::{errors::TransactionError, record::field::Value};

use super::{Scan, ScanControl};

pub struct ProjectScan {
    base_scan: Box<Scan>,
    fields: Vec<String>,
}

impl ProjectScan {
    pub fn new(base_scan: Scan, fields: Vec<String>) -> Self {
        for field in &fields {
            assert!(base_scan.has_field(field));
        }

        ProjectScan {
            base_scan: Box::new(base_scan),
            fields,
        }
    }
}

impl ScanControl for ProjectScan {
    fn before_first(&mut self) -> Result<(), TransactionError> {
        self.base_scan.before_first()
    }

    fn after_last(&mut self) -> Result<(), TransactionError> {
        self.base_scan.after_last()
    }

    fn previous(&mut self) -> Result<bool, TransactionError> {
        self.base_scan.previous()
    }

    fn next(&mut self) -> Result<bool, TransactionError> {
        self.base_scan.next()
    }

    fn get_i32(&mut self, field_name: &str) -> Result<Option<i32>, TransactionError> {
        assert!(self.fields.contains(&field_name.to_string()));
        self.base_scan.get_i32(field_name)
    }

    fn get_string(&mut self, field_name: &str) -> Result<Option<String>, TransactionError> {
        assert!(self.fields.contains(&field_name.to_string()));
        self.base_scan.get_string(field_name)
    }

    fn get_value(&mut self, field_name: &str) -> Result<Value, TransactionError> {
        assert!(self.fields.contains(&field_name.to_string()));
        self.base_scan.get_value(field_name)
    }

    fn has_field(&self, field_name: &str) -> bool {
        self.fields.contains(&field_name.to_string())
    }
}

#[cfg(test)]
mod tests {

    use std::sync::{Arc, Mutex};

    use crate::{
        db::SimpleDB,
        record::{layout::Layout, schema::Schema},
        scan::table_scan::TableScan,
    };

    use super::*;

    #[test]
    fn test_project_scan() -> Result<(), TransactionError> {
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);
        schema.add_i32_field("C");
        let layout = Arc::new(Layout::new(schema));

        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let mut table_scan = TableScan::new(tx.clone(), "testtable", layout.clone())?;
        table_scan.before_first()?;
        for i in 0..50 {
            table_scan.insert()?;
            table_scan.set_i32("A", i)?;
            table_scan.set_string("B", &i.to_string())?;
            table_scan.set_i32("C", i + 2)?;
        }

        let mut project_scan = Scan::ProjectScan(ProjectScan::new(
            Scan::from(table_scan),
            vec!["A".to_string(), "C".to_string()],
        ));
        project_scan.before_first()?;
        for i in 0..50 {
            assert!(project_scan.next()?);
            assert!(project_scan.has_field("A"));
            assert!(!project_scan.has_field("B"));
            assert!(project_scan.has_field("C"));
            assert_eq!(project_scan.get_i32("A")?, Some(i));
            assert_eq!(project_scan.get_i32("C")?, Some(i + 2));
        }
        drop(project_scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }

    #[test]
    fn test_project_scan_previous() -> Result<(), TransactionError> {
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_i32_field("C");
        let layout = Arc::new(Layout::new(schema));

        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        let mut table_scan = TableScan::new(tx.clone(), "testtable2", layout.clone())?;
        table_scan.before_first()?;
        for i in 0..10 {
            table_scan.insert()?;
            table_scan.set_i32("A", i)?;
            table_scan.set_i32("C", i + 1)?;
        }

        let mut scan = Scan::ProjectScan(ProjectScan::new(
            Scan::from(table_scan),
            vec!["A".to_string(), "C".to_string()],
        ));
        scan.after_last()?;
        for i in (0..10).rev() {
            assert!(scan.previous()?);
            assert_eq!(scan.get_i32("A")?, Some(i));
            assert_eq!(scan.get_i32("C")?, Some(i + 1));
        }
        assert!(!scan.previous()?);
        drop(scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }
}
