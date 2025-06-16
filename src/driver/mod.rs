use embedded::{
    EmbeddedConnection, EmbeddedDriver, EmbeddedMetadata, EmbeddedResultSet, EmbeddedStatement,
};
use enum_dispatch::enum_dispatch;

use network::{
    connection::NetworkConnection, driver::NetworkDriver, metadata::NetworkMetadata,
    result_set::NetworkResultSet, statement::NetworkStatement,
};
// use network::NetworkMetadata;

use crate::record::field::Type;

pub mod embedded;
pub mod network;

#[enum_dispatch]
pub enum Metadata {
    Embedded(EmbeddedMetadata),
    Network(NetworkMetadata),
}

#[enum_dispatch(Metadata)]
pub trait MetadataControl {
    fn get_column_count(&self) -> Result<usize, anyhow::Error>;
    fn get_column_name(&self, index: usize) -> Result<String, anyhow::Error>;
    fn get_column_display_size(&self, index: usize) -> Result<usize, anyhow::Error>;
    fn get_column_type(&self, index: usize) -> Result<Type, anyhow::Error>;
}

#[enum_dispatch]
pub enum ResultSet {
    Embedded(EmbeddedResultSet),
    NetworkResultSet(NetworkResultSet),
}

#[enum_dispatch(ResultSet)]
pub trait ResultSetControl {
    fn get_metadata(&self) -> Result<Metadata, anyhow::Error>;
    fn next(&mut self) -> Result<bool, anyhow::Error>;
    fn before_first(&mut self) -> Result<(), anyhow::Error>;
    fn absolute(&mut self, n: usize) -> Result<bool, anyhow::Error>;
    fn get_i32(&mut self, column_name: &str) -> Result<i32, anyhow::Error>;
    fn get_string(&mut self, column_name: &str) -> Result<String, anyhow::Error>;
    fn close(&mut self) -> Result<(), anyhow::Error>;
}

#[enum_dispatch]
pub enum Statement {
    Embedded(EmbeddedStatement),
    Network(NetworkStatement),
}

#[enum_dispatch(Statement)]
pub trait StatementControl {
    fn execute_query(&mut self, command: &str) -> Result<ResultSet, anyhow::Error>;
    fn execute_update(&mut self, command: &str) -> Result<usize, anyhow::Error>;
}

#[enum_dispatch]
pub enum Connection {
    Embedded(EmbeddedConnection),
    Network(NetworkConnection),
}

#[enum_dispatch(Connection)]
pub trait ConnectionControl {
    fn create_statement(&self) -> Result<Statement, anyhow::Error>;
    fn close(&mut self) -> Result<(), anyhow::Error>;
    fn commit(&mut self) -> Result<(), anyhow::Error>;
    fn rollback(&mut self) -> Result<(), anyhow::Error>;
}

#[enum_dispatch]
pub enum Driver {
    Embedded(EmbeddedDriver),
    Network(NetworkDriver),
}

#[enum_dispatch(Driver)]
pub trait DriverControl {
    fn connect(&self, db_url: &str) -> Result<(String, Connection), anyhow::Error>;
}
