use crate::{errors::TransactionError, parser::expression::Expression, record::field::Value};

use super::{Scan, ScanControl};

pub struct ExtendScan {
    base_scan: Box<Scan>,
    expression: Expression,
    field_name: String,
}

impl ExtendScan {
    pub fn new(base_scan: Scan, expression: Expression, field_name: &str) -> Self {
        Self {
            base_scan: Box::new(base_scan),
            expression,
            field_name: field_name.to_string(),
        }
    }
}

impl ScanControl for ExtendScan {
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
        match self.get_value(field_name)? {
            Value::I32(i) => Ok(Some(i)),
            Value::Null => Ok(None),
            _ => panic!("Field {} is not an integer", field_name),
        }
    }

    fn get_string(&mut self, field_name: &str) -> Result<Option<String>, TransactionError> {
        match self.get_value(field_name)? {
            Value::String(s) => Ok(Some(s)),
            Value::Null => Ok(None),
            _ => panic!("Field {} is not a string", field_name),
        }
    }

    fn get_value(&mut self, field_name: &str) -> Result<Value, TransactionError> {
        if field_name == self.field_name {
            self.expression.evaluate(&mut self.base_scan)
        } else {
            self.base_scan.get_value(field_name)
        }
    }

    fn has_field(&self, field_name: &str) -> bool {
        field_name == self.field_name || self.base_scan.has_field(field_name)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{
        db::SimpleDB,
        parser::expression::Expression,
        record::{layout::Layout, schema::Schema},
        scan::table_scan::TableScan,
        scan::ScanControl,
    };

    use super::*;

    #[test]
    fn test_extend_scan() -> Result<(), TransactionError> {
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_i32_field("B");
        let layout = Arc::new(Layout::new(schema));

        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        let mut table_scan = TableScan::new(tx.clone(), "t", layout.clone())?;
        table_scan.before_first()?;
        for i in 0..10 {
            table_scan.insert()?;
            table_scan.set_i32("A", i)?;
            table_scan.set_i32("B", i + 1)?;
        }

        let expression = Expression::Sub(
            Box::new(Expression::Field("B".to_string())),
            Box::new(Expression::I32Constant(1)),
        );
        let mut scan = ExtendScan::new(Scan::from(table_scan), expression, "C");
        scan.before_first()?;
        for i in 0..10 {
            assert!(scan.next()?);
            assert_eq!(scan.get_i32("A")?, Some(i));
            assert_eq!(scan.get_i32("C")?, Some(i));
        }
        assert!(!scan.next()?);
        Ok(())
    }
}
