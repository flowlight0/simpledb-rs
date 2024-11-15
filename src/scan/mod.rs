use crate::record::field::Value;

pub trait Scan {
    fn before_first(&mut self) -> Result<(), anyhow::Error>;
    fn next(&mut self) -> Result<bool, anyhow::Error>;
    fn get_i32(&mut self, field_name: &str) -> Result<i32, anyhow::Error>;
    fn get_string(&mut self, field_name: &str) -> Result<String, anyhow::Error>;
    fn get_value(&mut self, field_name: &str) -> Result<Value, anyhow::Error>;
    fn has_field(&self, field_name: &str) -> bool;

    // Update operations
    #[allow(unused_variables)]
    fn set_i32(&mut self, field_name: &str, value: i32) -> Result<(), anyhow::Error> {
        Err(anyhow::anyhow!("Update operation is not supported"))
    }

    #[allow(unused_variables)]
    fn set_string(&mut self, field_name: &str, value: &str) -> Result<(), anyhow::Error> {
        Err(anyhow::anyhow!("Update operation is not supported"))
    }

    #[allow(unused_variables)]
    fn delete(&mut self) -> Result<(), anyhow::Error> {
        Err(anyhow::anyhow!("Update operation is not supported"))
    }

    #[allow(unused_variables)]
    fn insert(&mut self) -> Result<(), anyhow::Error> {
        Err(anyhow::anyhow!("Update operation is not supported"))
    }
}

pub mod product_scan;
pub mod project_scan;
pub mod select_scan;
pub mod table_scan;
