use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tonic::{transport::Endpoint, Request, Response};

use crate::{
    driver::{
        embedded::{EmbeddedConnection, EmbeddedDriver},
        network::connection::NetworkConnection,
        Connection, DriverControl,
    },
    proto::simpledb::{
        driver_service_server::DriverService, DriverCreateConnectionRequest,
        DriverCreateConnectionResponse,
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

pub struct NetworkDriver {}

impl NetworkDriver {
    pub fn new() -> Self {
        NetworkDriver {}
    }
}

impl DriverControl for NetworkDriver {
    fn connect(&self, db_url: &str) -> Result<(String, Connection), anyhow::Error> {
        if let Some(idx) = db_url.find("//") {
            let endpoint = Endpoint::from_shared(format!("http://{}:50051", &db_url[idx + 2..]))?;
            return Ok((
                "".to_string(),
                Connection::Network(NetworkConnection::new(endpoint)?),
            ));
        } else {
            panic!("Invalid URL");
        }
    }
}
