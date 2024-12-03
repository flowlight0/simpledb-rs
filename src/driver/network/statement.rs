use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::runtime::Runtime;
use tonic::{Request, Response, Status};

use crate::driver::embedded::{EmbeddedResultSet, EmbeddedStatement};
use crate::driver::{ResultSet, StatementControl};
use crate::proto::simpledb::statement_service_client::StatementServiceClient;
use crate::proto::simpledb::statement_service_server::StatementService;
use crate::proto::simpledb::{
    StatementExecuteQueryRequest, StatementExecuteQueryResponse, StatementExecuteUpdateRequest,
    StatementExecuteUpdateResponse,
};

use super::connection::{NetworkConnection, RemoteConnection};
use super::result_set::{NetworkResultSet, RemoteResultSet};

// Each embedded statement can have multiple result sets.
pub struct RemoteStatement {
    pub(crate) embedded_result_set_dict: Arc<Mutex<HashMap<u64, EmbeddedResultSet>>>,
    embedded_statement_dict: Arc<Mutex<HashMap<u64, EmbeddedStatement>>>,
}

impl RemoteStatement {
    pub fn new(connection: &RemoteConnection) -> Self {
        Self {
            embedded_result_set_dict: Arc::new(Mutex::new(HashMap::new())),
            embedded_statement_dict: connection.embedded_statement_dict.clone(),
        }
    }

    pub fn create_remote_result_set(&self) -> RemoteResultSet {
        RemoteResultSet::new(self)
    }
}

#[tonic::async_trait]
impl StatementService for RemoteStatement {
    async fn execute_query(
        &self,
        request: Request<StatementExecuteQueryRequest>,
    ) -> Result<Response<StatementExecuteQueryResponse>, Status> {
        let request = request.into_inner();
        let statement_id = request.id;
        let embedded_result_set = {
            let mut lock = self.embedded_statement_dict.lock().unwrap();
            let embedded_statement = lock
                .get_mut(&statement_id)
                .expect(&format!("Unknown statement_id: {}", statement_id));

            let result_set = embedded_statement.execute_query(&request.command).unwrap();
            if let ResultSet::Embedded(result_set) = result_set {
                result_set
            } else {
                panic!("Unexpected result set type");
            }
        };

        let mut lock = self.embedded_result_set_dict.lock().unwrap();
        let mut result_set_id = 0;
        while lock.contains_key(&result_set_id) {
            result_set_id += 1;
        }
        lock.insert(result_set_id, embedded_result_set);
        Ok(Response::new(StatementExecuteQueryResponse {
            result_set_id,
        }))
    }
    async fn execute_update(
        &self,
        request: Request<StatementExecuteUpdateRequest>,
    ) -> Result<Response<StatementExecuteUpdateResponse>, Status> {
        let request = request.into_inner();
        let statement_id = request.id;
        let mut lock = self.embedded_statement_dict.lock().unwrap();
        let embedded_statement = lock
            .get_mut(&statement_id)
            .expect(&format!("Unknown statement_id: {}", statement_id));

        let num_updated_rows = embedded_statement.execute_update(&request.command).unwrap() as u64;
        Ok(Response::new(StatementExecuteUpdateResponse {
            num_updated_rows,
        }))
    }
}

pub struct NetworkStatement {
    connection: NetworkConnection,
    statement_id: u64,
    run_time: Arc<Runtime>,
}

impl NetworkStatement {
    pub fn new(connection: NetworkConnection, statement_id: u64, run_time: Arc<Runtime>) -> Self {
        Self {
            connection,
            statement_id,
            run_time,
        }
    }
}

impl StatementControl for NetworkStatement {
    fn execute_query(&mut self, sql: &str) -> Result<ResultSet, anyhow::Error> {
        let mut client = StatementServiceClient::new(self.connection.get_channel());
        let request = Request::new(StatementExecuteQueryRequest {
            id: self.statement_id,
            command: sql.to_string(),
        });
        let response: StatementExecuteQueryResponse = self
            .run_time
            .block_on(client.execute_query(request))
            .unwrap()
            .into_inner();

        Ok(ResultSet::NetworkResultSet(NetworkResultSet::new(
            self.connection.clone(),
            response.result_set_id,
            self.run_time.clone(),
        )))
    }

    fn execute_update(&mut self, sql: &str) -> Result<usize, anyhow::Error> {
        let mut client = StatementServiceClient::new(self.connection.get_channel());
        let request = Request::new(StatementExecuteUpdateRequest {
            id: self.statement_id,
            command: sql.to_string(),
        });
        let response: StatementExecuteUpdateResponse = self
            .run_time
            .block_on(client.execute_update(request))
            .unwrap()
            .into_inner();

        Ok(response.num_updated_rows as usize)
    }
}
