use enum_dispatch::enum_dispatch;

use crate::{errors::TransactionError, record::field::Value, scan::Scan};

use super::{
    avg_function::AvgFn, count_function::CountFn, max_function::MaxFn, min_function::MinFn,
    sum_function::SumFn,
};

#[derive(Clone, Debug, PartialEq, Eq)]
#[enum_dispatch]
pub enum AggregationFn {
    Max(MaxFn),
    Count(CountFn),
    Min(MinFn),
    Sum(SumFn),
    Avg(AvgFn),
}

#[enum_dispatch(AggregationFn)]
pub trait AggregationFnControl {
    fn process_first(&mut self, scan: &mut Scan) -> Result<(), TransactionError>;
    fn process_next(&mut self, scan: &mut Scan) -> Result<(), TransactionError>;
    fn get_field_name(&self) -> &str;
    fn get_value(&self) -> Option<Value>;
}

impl AggregationFn {
    pub fn with_alias(self, alias: &str) -> Self {
        match self {
            AggregationFn::Max(f) => AggregationFn::Max(f.with_alias(alias)),
            AggregationFn::Count(f) => AggregationFn::Count(f.with_alias(alias)),
            AggregationFn::Min(f) => AggregationFn::Min(f.with_alias(alias)),
            AggregationFn::Sum(f) => AggregationFn::Sum(f.with_alias(alias)),
            AggregationFn::Avg(f) => AggregationFn::Avg(f.with_alias(alias)),
        }
    }

    pub fn input_field_name(&self) -> &str {
        match self {
            AggregationFn::Max(f) => f.input_field_name(),
            AggregationFn::Count(f) => f.input_field_name(),
            AggregationFn::Min(f) => f.input_field_name(),
            AggregationFn::Sum(f) => f.input_field_name(),
            AggregationFn::Avg(f) => f.input_field_name(),
        }
    }
}
