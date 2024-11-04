pub trait Scan {
    fn before_first(&mut self) -> Result<(), anyhow::Error>;
    fn next(&mut self) -> Result<bool, anyhow::Error>;
    fn get_i32(&mut self, field_name: &str) -> Result<i32, anyhow::Error>;
    fn get_string(&mut self, field_name: &str) -> Result<String, anyhow::Error>;
}
pub trait UpdateScan: Scan {
    fn set_i32(&mut self, field_name: &str, value: i32) -> Result<(), anyhow::Error>;
    fn set_string(&mut self, field_name: &str, value: &str) -> Result<(), anyhow::Error>;
    fn delete(&mut self) -> Result<(), anyhow::Error>;
    fn insert(&mut self) -> Result<(), anyhow::Error>;
}
pub mod table_scan;
