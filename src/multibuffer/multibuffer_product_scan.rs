use crate::{errors::TransactionError, record::field::Value, scan::ScanControl};

pub struct MultiBufferProductScan {}

impl ScanControl for MultiBufferProductScan {
    fn before_first(&mut self) -> Result<(), TransactionError> {
        todo!()
    }

    fn next(&mut self) -> Result<bool, TransactionError> {
        todo!()
    }

    fn get_i32(&mut self, field_name: &str) -> Result<i32, TransactionError> {
        todo!()
    }

    fn get_string(&mut self, field_name: &str) -> Result<String, TransactionError> {
        todo!()
    }

    fn get_value(&mut self, field_name: &str) -> Result<Value, TransactionError> {
        todo!()
    }

    fn has_field(&self, field_name: &str) -> bool {
        todo!()
    }
}
