use crate::{
    errors::TransactionError,
    record::field::Value,
    scan::{Scan, ScanControl},
};

use super::aggregation_function::AggregationFnControl;

#[derive(Clone)]
pub struct CountFn {
    count: i32,
    field_name: String,
}

impl CountFn {
    pub fn new(field_name: &str) -> Self {
        Self {
            count: 0,
            field_name: field_name.to_string(),
        }
    }
}

impl AggregationFnControl for CountFn {
    fn process_first(&mut self, scan: &mut Scan) -> Result<(), TransactionError> {
        self.count = 0;
        let value = scan.get_value(&self.field_name)?;
        if value != Value::Null {
            self.count = 1;
        }
        Ok(())
    }

    fn process_next(&mut self, scan: &mut Scan) -> Result<(), TransactionError> {
        let value = scan.get_value(&self.field_name)?;
        if value != Value::Null {
            self.count += 1;
        }
        Ok(())
    }

    fn get_field_name(&self) -> &str {
        &self.field_name
    }

    fn get_value(&self) -> Option<Value> {
        Some(Value::I32(self.count))
    }
}
