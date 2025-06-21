use crate::{
    errors::TransactionError,
    record::field::Value,
    scan::{Scan, ScanControl},
};

use super::aggregation_function::AggregationFnControl;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaxFn {
    max_value: Option<Value>,
    field_name: String,
}

impl MaxFn {
    pub fn new(field_name: &str) -> Self {
        MaxFn {
            max_value: None,
            field_name: field_name.to_string(),
        }
    }
}

impl AggregationFnControl for MaxFn {
    fn process_first(&mut self, scan: &mut Scan) -> Result<(), TransactionError> {
        let value = scan.get_value(&self.field_name)?;
        if value == Value::Null {
            self.max_value = None;
        } else {
            self.max_value = Some(value);
        }
        Ok(())
    }

    fn process_next(&mut self, scan: &mut Scan) -> Result<(), TransactionError> {
        let new_value = scan.get_value(&self.field_name)?;
        if new_value == Value::Null {
            return Ok(());
        }
        if let Some(max) = &mut self.max_value {
            if new_value > *max {
                *max = new_value;
            }
        } else {
            self.max_value = Some(new_value);
        }
        Ok(())
    }

    fn get_field_name(&self) -> &str {
        &self.field_name
    }

    fn get_value(&self) -> Option<Value> {
        self.max_value.clone()
    }
}
