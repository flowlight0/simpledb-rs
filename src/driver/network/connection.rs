use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::runtime::{Builder, Runtime};
use tonic::transport::{Channel, Endpoint};
use tonic::{Request, Response, Status};

use crate::driver::embedded::{EmbeddedConnection, EmbeddedStatement};
use crate::driver::{ConnectionControl, Statement};
use crate::proto::simpledb::connection_service_client::ConnectionServiceClient;
use crate::proto::simpledb::connection_service_server::ConnectionService;
use crate::proto::simpledb::{
    ConnectionCloseRequest, ConnectionCloseResponse, ConnectionCommitRequest,
    ConnectionCommitResponse, ConnectionCreateStatementRequest, ConnectionCreateStatementResponse,
    ConnectionRollbackRequest, ConnectionRollbackResponse,
};

use super::statement::NetworkStatement;

struct RemoteConnection {
    embedded_connection_dict: Arc<Mutex<HashMap<u64, EmbeddedConnection>>>,
    embedded_statement_dict: Arc<Mutex<HashMap<u64, EmbeddedStatement>>>,
}

#[tonic::async_trait]
impl ConnectionService for RemoteConnection {
    // The client accesses the remote statement with a returned statement_id.
    async fn create_statement(
        &self,
        request: Request<ConnectionCreateStatementRequest>,
    ) -> Result<Response<ConnectionCreateStatementResponse>, Status> {
        let embedded_statement = {
            let connection_id = request.into_inner().id;
            let mut lock = self.embedded_connection_dict.lock().unwrap();
            let connection = lock
                .get_mut(&connection_id)
                .expect(&format!("Unknown connection_id: {}", connection_id));

            let statement = connection.create_statement().unwrap();
            if let Statement::Embedded(embedded_statement) = statement {
                embedded_statement
            } else {
                panic!("Unexpected statement type");
            }
        };

        let mut statement_id = 0;
        let mut lock = self.embedded_statement_dict.lock().unwrap();
        while lock.contains_key(&statement_id) {
            statement_id += 1;
        }

        lock.insert(statement_id, embedded_statement);
        Ok(Response::new(ConnectionCreateStatementResponse {
            statement_id,
        }))
    }

    async fn close(
        &self,
        request: Request<ConnectionCloseRequest>,
    ) -> Result<Response<ConnectionCloseResponse>, Status> {
        let connection_id = request.into_inner().id;
        let mut lock = self.embedded_connection_dict.lock().unwrap();
        let connection = lock
            .get_mut(&connection_id)
            .expect(&format!("Unknown connection_id: {}", connection_id));
        connection.close().unwrap();

        self.embedded_connection_dict
            .lock()
            .unwrap()
            .remove(&connection_id);
        Ok(Response::new(ConnectionCloseResponse {}))
    }

    async fn commit(
        &self,
        request: Request<ConnectionCommitRequest>,
    ) -> Result<Response<ConnectionCommitResponse>, Status> {
        let connection_id = request.into_inner().id;
        let mut lock = self.embedded_connection_dict.lock().unwrap();
        let connection = lock
            .get_mut(&connection_id)
            .expect(&format!("Unknown connection_id: {}", connection_id));
        connection.commit().unwrap();
        Ok(Response::new(ConnectionCommitResponse {}))
    }

    async fn rollback(
        &self,
        request: Request<ConnectionRollbackRequest>,
    ) -> Result<Response<ConnectionRollbackResponse>, Status> {
        let connection_id = request.into_inner().id;
        let mut lock = self.embedded_connection_dict.lock().unwrap();
        let connection = lock
            .get_mut(&connection_id)
            .expect(&format!("Unknown connection_id: {}", connection_id));
        connection.rollback().unwrap();
        Ok(Response::new(ConnectionRollbackResponse {}))
    }
}

#[derive(Clone)]
pub struct NetworkConnection {
    channel: Channel,
    runtime: Arc<Runtime>,
    connection_id: u64,
}

impl NetworkConnection {
    pub fn new(endpoint: Endpoint) -> Result<Self, anyhow::Error> {
        let runtime = Builder::new_current_thread().enable_all().build()?;
        let channel = runtime.block_on(endpoint.connect())?;
        let connection_id = 0;

        Ok(Self {
            channel,
            runtime: Arc::new(runtime),
            connection_id,
        })
    }

    pub fn get_channel(&self) -> Channel {
        self.channel.clone()
    }
}

impl ConnectionControl for NetworkConnection {
    fn create_statement(&self) -> Result<Statement, anyhow::Error> {
        let mut client = ConnectionServiceClient::new(self.get_channel());
        let request = Request::new(ConnectionCreateStatementRequest {
            id: self.connection_id,
        });
        let response = self
            .runtime
            .block_on(client.create_statement(request))
            .unwrap()
            .into_inner();
        let statement_id = response.statement_id;

        Ok(Statement::Network(NetworkStatement::new(
            self.clone(),
            statement_id,
            self.runtime.clone(),
        )))
    }

    fn close(&mut self) -> Result<(), anyhow::Error> {
        let mut client = ConnectionServiceClient::new(self.get_channel());
        let request = Request::new(ConnectionCloseRequest {
            id: self.connection_id,
        });
        self.runtime.block_on(client.close(request))?;
        Ok(())
    }

    fn commit(&mut self) -> Result<(), anyhow::Error> {
        let mut client = ConnectionServiceClient::new(self.get_channel());
        let request = Request::new(ConnectionCommitRequest {
            id: self.connection_id,
        });
        self.runtime.block_on(client.commit(request))?;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), anyhow::Error> {
        let mut client = ConnectionServiceClient::new(self.get_channel());
        let request = Request::new(ConnectionRollbackRequest {
            id: self.connection_id,
        });
        self.runtime.block_on(client.rollback(request))?;
        Ok(())
    }
}
