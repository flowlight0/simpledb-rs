use simpledb_rs::driver::network::driver::RemoteDriver;
use simpledb_rs::proto::simpledb::connection_service_server::ConnectionServiceServer;
use simpledb_rs::proto::simpledb::driver_service_server::DriverServiceServer;
use simpledb_rs::proto::simpledb::metadata_service_server::MetadataServiceServer;
use simpledb_rs::proto::simpledb::result_set_service_server::ResultSetServiceServer;
use simpledb_rs::proto::simpledb::statement_service_server::StatementServiceServer;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:50051".parse()?;

    let remote_driver = RemoteDriver::new();
    let remote_connection = remote_driver.create_remote_connection();
    let remote_statement = remote_connection.create_remote_statement();
    let remote_result_set = remote_statement.create_remote_result_set();
    let remote_metadata = remote_result_set.create_remote_metadata();

    Server::builder()
        .add_service(DriverServiceServer::new(remote_driver))
        .add_service(ConnectionServiceServer::new(remote_connection))
        .add_service(StatementServiceServer::new(remote_statement))
        .add_service(ResultSetServiceServer::new(remote_result_set))
        .add_service(MetadataServiceServer::new(remote_metadata))
        .serve(addr)
        .await?;

    Ok(())
}
