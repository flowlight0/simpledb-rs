use crate::{record::field::Value, tx::errors::TransactionError};

pub trait Scan {
    fn before_first(&mut self) -> Result<(), TransactionError>;
    fn next(&mut self) -> Result<bool, TransactionError>;
    fn get_i32(&mut self, field_name: &str) -> Result<i32, TransactionError>;
    fn get_string(&mut self, field_name: &str) -> Result<String, TransactionError>;
    fn get_value(&mut self, field_name: &str) -> Result<Value, TransactionError>;
    fn has_field(&self, field_name: &str) -> bool;
    fn close(&mut self) -> Result<(), TransactionError>;

    // Update operations
    #[allow(unused_variables)]
    fn set_i32(&mut self, field_name: &str, value: i32) -> Result<(), TransactionError> {
        unimplemented!("Update operation is not supported")
    }

    #[allow(unused_variables)]
    fn set_string(&mut self, field_name: &str, value: &str) -> Result<(), TransactionError> {
        unimplemented!("Update operation is not supported")
    }

    #[allow(unused_variables)]
    fn set_value(&mut self, field_name: &str, value: &Value) -> Result<(), TransactionError> {
        unimplemented!("Update operation is not supported")
    }

    #[allow(unused_variables)]
    fn delete(&mut self) -> Result<(), TransactionError> {
        unimplemented!("Update operation is not supported")
    }

    #[allow(unused_variables)]
    fn insert(&mut self) -> Result<(), TransactionError> {
        unimplemented!("Update operation is not supported")
    }
}

pub mod product_scan;
pub mod project_scan;
pub mod select_scan;
pub mod table_scan;
