use crate::record::field::Type;

pub mod embedded;

pub trait Metadata {
    fn get_column_count(&self) -> usize;
    fn get_column_name(&self, index: usize) -> String;
    fn get_column_display_size(&self, index: usize) -> usize;
    fn get_column_type(&self, index: usize) -> Type;
}

pub trait ResultSet {
    fn get_metadata(&self) -> Box<dyn Metadata>;
    fn next(&mut self) -> Result<bool, anyhow::Error>;
    fn get_i32(&mut self, column_name: &str) -> Result<i32, anyhow::Error>;
    fn get_string(&mut self, column_name: &str) -> Result<String, anyhow::Error>;
    fn close(&mut self) -> Result<(), anyhow::Error>;
}

pub trait Statement {
    fn execute_query(&mut self, command: &str) -> Result<Box<dyn ResultSet>, anyhow::Error>;
    fn execute_update(&mut self, command: &str) -> Result<usize, anyhow::Error>;
}

pub trait ConnectionAdaptor {
    fn create_statement(&self) -> Result<Box<dyn Statement>, anyhow::Error>;
    fn close(&mut self) -> Result<(), anyhow::Error>;
    fn commit(&mut self) -> Result<(), anyhow::Error>;
    fn rollback(&mut self) -> Result<(), anyhow::Error>;
}
