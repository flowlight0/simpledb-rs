use crate::{
    errors::TransactionError,
    record::field::Value,
    scan::{Scan, ScanControl},
};

use super::aggregation_function::AggregationFnControl;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvgFn {
    sum: i64,
    count: i32,
    field_name: String,
    alias: Option<String>,
}

impl AvgFn {
    pub fn new(field_name: &str) -> Self {
        Self {
            sum: 0,
            count: 0,
            field_name: field_name.to_string(),
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

impl AggregationFnControl for AvgFn {
    fn process_first(&mut self, scan: &mut Scan) -> Result<(), TransactionError> {
        self.sum = 0;
        self.count = 0;
        match scan.get_value(&self.field_name)? {
            Value::I32(i) => {
                self.sum = i as i64;
                self.count = 1;
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
                self.count += 1;
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
        if self.count > 0 {
            Some(Value::I32((self.sum / self.count as i64) as i32))
        } else {
            None
        }
    }
}
