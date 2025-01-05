use enum_dispatch::enum_dispatch;

use crate::{errors::TransactionError, record::field::Value, scan::Scan};

use super::max_function::MaxFn;

#[derive(Clone)]
#[enum_dispatch]
pub enum AggregationFn {
    Max(MaxFn),
}

#[enum_dispatch(AggregationFn)]
pub trait AggregationFnControl {
    fn process_first(&mut self, scan: &mut Scan) -> Result<(), TransactionError>;
    fn process_next(&mut self, scan: &mut Scan) -> Result<(), TransactionError>;
    fn get_field_name(&self) -> &str;
    fn get_value(&self) -> Option<Value>;
}
