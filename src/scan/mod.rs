use enum_dispatch::enum_dispatch;
use product_scan::ProductScan;
use project_scan::ProjectScan;
use select_scan::SelectScan;
use table_scan::TableScan;

use crate::{
    errors::TransactionError,
    index::scan::{index_join_scan::IndexJoinScan, index_select_scan::IndexSelectScan},
    materialization::{
        group_by_scan::GroupByScan, merge_join_scan::MergeJoinScan, sort_scan::SortScan,
    },
    record::{field::Value, record_page::Slot},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordId(pub usize, pub usize); // block number, slot number

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordPointer(pub usize, pub Slot); // block number, slot

impl From<RecordId> for RecordPointer {
    fn from(record_id: RecordId) -> Self {
        RecordPointer(record_id.0, Slot::Index(record_id.1))
    }
}

impl From<RecordPointer> for RecordId {
    fn from(record_pointer: RecordPointer) -> Self {
        RecordId(record_pointer.0, record_pointer.1.index())
    }
}

#[enum_dispatch]
pub enum Scan {
    TableScan(TableScan),
    ProjectScan(ProjectScan),
    SelectScan(SelectScan),
    ProductScan(ProductScan),
    IndexSelectScan(IndexSelectScan),
    IndexJoinScan(IndexJoinScan),
    SortScan(SortScan),
    GroupByScan(GroupByScan),
    MergeJoinScan(MergeJoinScan),
}

#[enum_dispatch(Scan)]
pub trait ScanControl {
    fn before_first(&mut self) -> Result<(), TransactionError>;
    fn next(&mut self) -> Result<bool, TransactionError>;
    fn get_i32(&mut self, field_name: &str) -> Result<i32, TransactionError>;
    fn get_string(&mut self, field_name: &str) -> Result<String, TransactionError>;
    fn get_value(&mut self, field_name: &str) -> Result<Value, TransactionError>;
    fn has_field(&self, field_name: &str) -> bool;

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

    #[allow(unused_variables)]
    fn get_record_pointer(&self) -> RecordPointer {
        unimplemented!("Update operation is not supported")
    }

    #[allow(unused_variables)]
    fn move_to_record_pointer(
        &mut self,
        record_pointer: &RecordPointer,
    ) -> Result<(), TransactionError> {
        unimplemented!("Update operation is not supported")
    }
}

pub mod product_scan;
pub mod project_scan;
pub mod select_scan;
pub mod table_scan;
