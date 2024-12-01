use embedded::{EmbeddedMetadata, EmbeddedResultSet, EmbeddedStatement};
use enum_dispatch::enum_dispatch;
// use network::NetworkMetadata;

use crate::record::field::Type;

pub mod embedded;
pub mod network;

#[enum_dispatch]
pub enum Metadata {
    Embedded(EmbeddedMetadata),
    // Network(NetworkMetadata),
}

#[enum_dispatch(Metadata)]
pub trait MetadataControl {
    fn get_column_count(&self) -> usize;
    fn get_column_name(&self, index: usize) -> String;
    fn get_column_display_size(&self, index: usize) -> usize;
    fn get_column_type(&self, index: usize) -> Type;
}

#[enum_dispatch]
pub enum ResultSet {
    Embedded(EmbeddedResultSet),
    // Network(network::NetworkResultSet),
}

#[enum_dispatch(ResultSet)]
pub trait ResultSetControl {
    fn get_metadata(&self) -> Metadata;
    fn next(&mut self) -> Result<bool, anyhow::Error>;
    fn get_i32(&mut self, column_name: &str) -> Result<i32, anyhow::Error>;
    fn get_string(&mut self, column_name: &str) -> Result<String, anyhow::Error>;
    fn close(&mut self) -> Result<(), anyhow::Error>;
}

#[enum_dispatch]
pub enum Statement {
    Embedded(EmbeddedStatement),
}

#[enum_dispatch(Statement)]
pub trait StatementControl {
    fn execute_query(&mut self, command: &str) -> Result<ResultSet, anyhow::Error>;
    fn execute_update(&mut self, command: &str) -> Result<usize, anyhow::Error>;
}

pub trait ConnectionAdaptor {
    fn create_statement(&self) -> Result<Statement, anyhow::Error>;
    fn close(&mut self) -> Result<(), anyhow::Error>;
    fn commit(&mut self) -> Result<(), anyhow::Error>;
    fn rollback(&mut self) -> Result<(), anyhow::Error>;
}

pub trait Driver {
    fn connect(&self, db_url: &str) -> Result<(String, Box<dyn ConnectionAdaptor>), anyhow::Error>;
}
