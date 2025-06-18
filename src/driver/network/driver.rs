use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::runtime::{Builder, Runtime};
use tonic::{transport::Endpoint, Request, Response};

use crate::{
    driver::{
        embedded::{EmbeddedConnection, EmbeddedDriver},
        network::connection::NetworkConnection,
        Connection, DriverControl,
    },
    proto::simpledb::{
        driver_service_client::DriverServiceClient, driver_service_server::DriverService,
        DriverCreateConnectionRequest, DriverCreateConnectionResponse,
    },
};

use super::connection::RemoteConnection;

pub struct RemoteDriver {
    // The driver does not need to maintain any state.
    embedded_driver: Arc<Mutex<EmbeddedDriver>>,
    pub(crate) embedded_connection_dict: Arc<Mutex<HashMap<u64, EmbeddedConnection>>>,
}

impl RemoteDriver {
    pub fn new() -> Self {
        Self {
            embedded_driver: Arc::new(Mutex::new(EmbeddedDriver::new())),
            embedded_connection_dict: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn create_remote_connection(&self) -> RemoteConnection {
        RemoteConnection::new(self)
    }
}

#[tonic::async_trait]
impl DriverService for RemoteDriver {
    async fn create_connection(
        &self,
        request: Request<DriverCreateConnectionRequest>,
    ) -> Result<Response<DriverCreateConnectionResponse>, tonic::Status> {
        dbg!(&request);
        let embedded_connection = {
            let (_, embedded_connection) = self
                .embedded_driver
                .lock()
                .unwrap()
                .connect(&request.into_inner().url)
                .unwrap();
            if let Connection::Embedded(embedded_connection) = embedded_connection {
                embedded_connection
            } else {
                panic!("Unexpected connection type");
            }
        };
        let mut connection_id = 0;
        let mut lock = self.embedded_connection_dict.lock().unwrap();
        while lock.contains_key(&connection_id) {
            connection_id += 1;
        }
        lock.insert(connection_id, embedded_connection);
        Ok(Response::new(DriverCreateConnectionResponse {
            connection_id,
        }))
    }
}

pub struct NetworkDriver {
    runtime: Arc<Runtime>,
}

impl NetworkDriver {
    pub fn new() -> Self {
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let runtime = Arc::new(runtime);
        NetworkDriver { runtime }
    }
}

impl DriverControl for NetworkDriver {
    fn connect(&self, db_url: &str) -> Result<(String, Connection), anyhow::Error> {
        if let Some(idx) = db_url.find("//") {
            let db_url = db_url[idx + 2..].trim();
            let url = format!("http://{}:50051", db_url);
            dbg!(&url);
            let endpoint = Endpoint::from_shared(url)?;
            let channel = self.runtime.block_on(endpoint.connect())?;
            let mut client = DriverServiceClient::new(channel.clone());
            let response = self
                .runtime
                .block_on(client.create_connection(DriverCreateConnectionRequest {
                    url: db_url.to_string(),
                }))?
                .into_inner();
            let connection_id = response.connection_id;

            return Ok((
                "".to_string(),
                Connection::Network(NetworkConnection::new(
                    self.runtime.clone(),
                    channel,
                    connection_id,
                )?),
            ));
        } else {
            panic!("Invalid URL");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{ConnectionControl, MetadataControl, ResultSetControl, StatementControl};
    use crate::proto::simpledb::{
        connection_service_server::ConnectionServiceServer,
        driver_service_server::DriverServiceServer, metadata_service_server::MetadataServiceServer,
        result_set_service_server::ResultSetServiceServer,
        statement_service_server::StatementServiceServer,
    };
    use std::thread;
    use std::time::Duration;
    use tokio::runtime::Runtime;
    use tokio::sync::oneshot;
    use tonic::transport::Server;

    #[test]
    fn test_network_driver_basic_flow() -> Result<(), anyhow::Error> {
        let temp_dir = tempfile::tempdir()?.into_path();

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let _server_thread = {
            let temp_dir = temp_dir.clone();
            thread::spawn(move || {
                std::env::set_current_dir(temp_dir).unwrap();
                let rt = Runtime::new().unwrap();
                rt.block_on(async move {
                    let addr = "127.0.0.1:50051".parse().unwrap();

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
                        .serve_with_shutdown(addr, async {
                            shutdown_rx.await.ok();
                        })
                        .await
                        .unwrap();
                });
            })
        };

        // give the server a moment to start
        thread::sleep(Duration::from_millis(100));

        let driver = NetworkDriver::new();
        let (_db_name, connection) = driver.connect("jdbc:simpledb://127.0.0.1")?;

        let mut statement = connection.create_statement()?;
        statement.execute_update("create table test (A I32, B VARCHAR(20))")?;
        statement.execute_update("insert into test (A, B) values (1, 'a')")?;
        statement.execute_update("insert into test (A, B) values (2, NULL)")?;

        let mut result_set = statement.execute_query("select B from test")?;
        let metadata = result_set.get_metadata()?;
        assert_eq!(metadata.get_column_count()?, 1);
        assert_eq!(metadata.get_column_name(0)?, "B");

        assert!(result_set.next()?);
        assert_eq!(result_set.get_string("B")?, Some("a".to_string()));
        assert!(result_set.next()?);
        assert_eq!(result_set.get_string("B")?, None);
        assert!(!result_set.next()?);
        result_set.close()?;

        // Drop the connection instead of explicitly closing to avoid
        // blocking on the server side during the test.
        drop(connection);

        shutdown_tx.send(()).ok();
        // Allow the server to shut down asynchronously.
        Ok(())
    }
}
