use crate::{
    errors::TransactionError,
    record::field::Value,
    scan::{Scan, ScanControl},
};

use super::aggregation_function::AggregationFnControl;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SumFn {
    sum: i64,
    field_name: String,
    has_value: bool,
    alias: Option<String>,
}

impl SumFn {
    pub fn new(field_name: &str) -> Self {
        Self {
            sum: 0,
            field_name: field_name.to_string(),
            has_value: false,
            alias: None,
        }
    }

    pub fn with_alias(mut self, alias: &str) -> Self {
        self.alias = Some(alias.to_string());
        self
    }

    pub fn input_field_name(&self) -> &str {
        &self.field_name
    }
}

impl AggregationFnControl for SumFn {
    fn process_first(&mut self, scan: &mut Scan) -> Result<(), TransactionError> {
        self.sum = 0;
        self.has_value = false;
        match scan.get_value(&self.field_name)? {
            Value::I32(i) => {
                self.sum = i as i64;
                self.has_value = true;
            }
            Value::Null => {}
            _ => panic!("Field {} is not an integer", self.field_name),
        }
        Ok(())
    }

    fn process_next(&mut self, scan: &mut Scan) -> Result<(), TransactionError> {
        match scan.get_value(&self.field_name)? {
            Value::I32(i) => {
                self.sum += i as i64;
                self.has_value = true;
            }
            Value::Null => {}
            _ => panic!("Field {} is not an integer", self.field_name),
        }
        Ok(())
    }

    fn get_field_name(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.field_name)
    }

    fn get_value(&self) -> Option<Value> {
        if self.has_value {
            Some(Value::I32(self.sum as i32))
        } else {
            None
        }
    }
}
