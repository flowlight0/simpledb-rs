use tonic::transport::Endpoint;

use crate::driver::{network::connection::NetworkConnection, Connection, DriverControl};

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
