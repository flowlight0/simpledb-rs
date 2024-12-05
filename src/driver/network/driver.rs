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
