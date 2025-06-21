use crate::{
    errors::TransactionError,
    record::field::Value,
    scan::{Scan, ScanControl},
};

use super::aggregation_function::AggregationFnControl;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MinFn {
    min_value: Option<Value>,
    field_name: String,
    alias: Option<String>,
}

impl MinFn {
    pub fn new(field_name: &str) -> Self {
        Self {
            min_value: None,
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

impl AggregationFnControl for MinFn {
    fn process_first(&mut self, scan: &mut Scan) -> Result<(), TransactionError> {
        let value = scan.get_value(&self.field_name)?;
        if value == Value::Null {
            self.min_value = None;
        } else {
            self.min_value = Some(value);
        }
        Ok(())
    }

    fn process_next(&mut self, scan: &mut Scan) -> Result<(), TransactionError> {
        let new_value = scan.get_value(&self.field_name)?;
        if new_value == Value::Null {
            return Ok(());
        }
        if let Some(min) = &mut self.min_value {
            if new_value < *min {
                *min = new_value;
            }
        } else {
            self.min_value = Some(new_value);
        }
        Ok(())
    }

    fn get_field_name(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.field_name)
    }

    fn get_value(&self) -> Option<Value> {
        self.min_value.clone()
    }
}
