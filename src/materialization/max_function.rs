use crate::{
    errors::TransactionError,
    record::field::Value,
    scan::{Scan, ScanControl},
};

use super::aggregation_function::AggregationFnControl;

#[derive(Clone)]
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
        self.max_value = Some(value);
        Ok(())
    }

    fn process_next(&mut self, scan: &mut Scan) -> Result<(), TransactionError> {
        let new_value = scan.get_value(&self.field_name)?;
        if self.max_value.is_some() {
            if &new_value > self.max_value.as_ref().unwrap() {
                self.max_value = Some(new_value);
            }
        } else {
            panic!("MaxFn::process_next() called before MaxFn::process_first()");
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
