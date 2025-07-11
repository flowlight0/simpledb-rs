use std::sync::Arc;
use std::{collections::HashMap, sync::Mutex};

use tokio::runtime::Runtime;
use tonic::{Request, Response, Result, Status};

use crate::driver::embedded::{EmbeddedMetadata, EmbeddedResultSet};
use crate::driver::{Metadata, ResultSetControl};
use crate::proto::simpledb::result_set_service_client::ResultSetServiceClient;
use crate::proto::simpledb::result_set_service_server::ResultSetService;
use crate::proto::simpledb::{
    ResultSetAbsoluteRequest, ResultSetAbsoluteResponse, ResultSetAfterLastRequest,
    ResultSetAfterLastResponse, ResultSetBeforeFirstRequest, ResultSetBeforeFirstResponse,
    ResultSetCloseRequest, ResultSetCloseResponse, ResultSetGetI32Request, ResultSetGetI32Response,
    ResultSetGetMetadataRequest, ResultSetGetMetadataResponse, ResultSetGetStringRequest,
    ResultSetGetStringResponse, ResultSetNextRequest, ResultSetNextResponse,
    ResultSetPreviousRequest, ResultSetPreviousResponse,
};

use super::connection::NetworkConnection;
use super::metadata::{NetworkMetadata, RemoteMetadata};
use super::statement::RemoteStatement;

pub struct RemoteResultSet {
    embedded_result_set_dict: Arc<Mutex<HashMap<u64, EmbeddedResultSet>>>,
    pub(crate) embedded_metadata_dict: Arc<Mutex<HashMap<u64, EmbeddedMetadata>>>,
}

impl RemoteResultSet {
    pub fn new(statement: &RemoteStatement) -> Self {
        Self {
            embedded_result_set_dict: statement.embedded_result_set_dict.clone(),
            embedded_metadata_dict: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn create_remote_metadata(&self) -> RemoteMetadata {
        RemoteMetadata::new(self)
    }
}

#[tonic::async_trait]
impl ResultSetService for RemoteResultSet {
    async fn get_metadata(
        &self,
        request: Request<ResultSetGetMetadataRequest>,
    ) -> Result<Response<ResultSetGetMetadataResponse>, Status> {
        let request = request.into_inner();
        let result_set_id = request.id;
        let embedded_metadata = {
            let lock = self.embedded_result_set_dict.lock().unwrap();
            let embedded_result_set = lock
                .get(&result_set_id)
                .expect(&format!("Unknown result_set_id: {}", result_set_id));
            let embedded_metadata = embedded_result_set.get_metadata().unwrap();
            if let Metadata::Embedded(embedded_metadata) = embedded_metadata {
                embedded_metadata
            } else {
                panic!("Unexpected metadata type");
            }
        };

        let mut embedded_metadata_dict = self.embedded_metadata_dict.lock().unwrap();
        embedded_metadata_dict.insert(result_set_id, embedded_metadata);
        Ok(Response::new(ResultSetGetMetadataResponse {}))
    }

    async fn next(
        &self,
        request: Request<ResultSetNextRequest>,
    ) -> Result<Response<ResultSetNextResponse>, Status> {
        let request = request.into_inner();
        let result_set_id = request.id;
        let mut lock = self.embedded_result_set_dict.lock().unwrap();
        let embedded_result_set = lock
            .get_mut(&result_set_id)
            .expect(&format!("Unknown result_set_id: {}", result_set_id));
        let has_next = embedded_result_set.next().unwrap();
        Ok(Response::new(ResultSetNextResponse { has_next }))
    }

    async fn previous(
        &self,
        request: Request<ResultSetPreviousRequest>,
    ) -> Result<Response<ResultSetPreviousResponse>, Status> {
        let request = request.into_inner();
        let result_set_id = request.id;
        let mut lock = self.embedded_result_set_dict.lock().unwrap();
        let embedded_result_set = lock
            .get_mut(&result_set_id)
            .expect(&format!("Unknown result_set_id: {}", result_set_id));
        let has_prev = embedded_result_set.previous().unwrap();
        Ok(Response::new(ResultSetPreviousResponse { has_prev }))
    }

    async fn get_i32(
        &self,
        request: Request<ResultSetGetI32Request>,
    ) -> Result<Response<ResultSetGetI32Response>, Status> {
        let request = request.into_inner();
        let result_set_id = request.id;
        let column_name = request.column_name;
        let mut lock = self.embedded_result_set_dict.lock().unwrap();
        let embedded_result_set = lock
            .get_mut(&result_set_id)
            .expect(&format!("Unknown result_set_id: {}", result_set_id));
        let value_opt = embedded_result_set.get_i32(&column_name).unwrap();
        let (value, was_null) = match value_opt {
            Some(v) => (v, false),
            None => (0, true),
        };
        Ok(Response::new(ResultSetGetI32Response { value, was_null }))
    }

    async fn get_string(
        &self,
        request: Request<ResultSetGetStringRequest>,
    ) -> Result<Response<ResultSetGetStringResponse>, Status> {
        let request = request.into_inner();
        let result_set_id = request.id;
        let column_name = request.column_name;
        let mut lock = self.embedded_result_set_dict.lock().unwrap();
        let embedded_result_set = lock
            .get_mut(&result_set_id)
            .expect(&format!("Unknown result_set_id: {}", result_set_id));
        let value_opt = embedded_result_set.get_string(&column_name).unwrap();
        let (value, was_null) = match value_opt {
            Some(v) => (v, false),
            None => (String::new(), true),
        };
        Ok(Response::new(ResultSetGetStringResponse {
            value,
            was_null,
        }))
    }

    async fn before_first(
        &self,
        request: Request<ResultSetBeforeFirstRequest>,
    ) -> Result<Response<ResultSetBeforeFirstResponse>, Status> {
        let request = request.into_inner();
        let result_set_id = request.id;
        let mut lock = self.embedded_result_set_dict.lock().unwrap();
        let embedded_result_set = lock
            .get_mut(&result_set_id)
            .expect(&format!("Unknown result_set_id: {}", result_set_id));
        embedded_result_set.before_first().unwrap();
        Ok(Response::new(ResultSetBeforeFirstResponse {}))
    }

    async fn after_last(
        &self,
        request: Request<ResultSetAfterLastRequest>,
    ) -> Result<Response<ResultSetAfterLastResponse>, Status> {
        let request = request.into_inner();
        let result_set_id = request.id;
        let mut lock = self.embedded_result_set_dict.lock().unwrap();
        let embedded_result_set = lock
            .get_mut(&result_set_id)
            .expect(&format!("Unknown result_set_id: {}", result_set_id));
        embedded_result_set.after_last().unwrap();
        Ok(Response::new(ResultSetAfterLastResponse {}))
    }

    async fn absolute(
        &self,
        request: Request<ResultSetAbsoluteRequest>,
    ) -> Result<Response<ResultSetAbsoluteResponse>, Status> {
        let request = request.into_inner();
        let result_set_id = request.id;
        let n = request.n as usize;
        let mut lock = self.embedded_result_set_dict.lock().unwrap();
        let embedded_result_set = lock
            .get_mut(&result_set_id)
            .expect(&format!("Unknown result_set_id: {}", result_set_id));
        let has_row = embedded_result_set.absolute(n).unwrap();
        Ok(Response::new(ResultSetAbsoluteResponse { has_row }))
    }

    async fn close(
        &self,
        request: Request<ResultSetCloseRequest>,
    ) -> Result<Response<ResultSetCloseResponse>, Status> {
        let request = request.into_inner();
        let result_set_id = request.id;

        // Close the embedded result set
        {
            let mut lock = self.embedded_result_set_dict.lock().unwrap();
            let embedded_result_set = lock
                .get_mut(&result_set_id)
                .expect(&format!("Unknown result_set_id: {}", result_set_id));
            embedded_result_set.close().unwrap();
        }

        // Remove the embedded result set
        {
            let mut lock = self.embedded_result_set_dict.lock().unwrap();
            lock.remove(&result_set_id);
        }
        Ok(Response::new(ResultSetCloseResponse {}))
    }
}

pub struct NetworkResultSet {
    connection: NetworkConnection,
    result_set_id: u64,
    run_time: Arc<Runtime>,
}

impl NetworkResultSet {
    pub fn new(connection: NetworkConnection, result_set_id: u64, run_time: Arc<Runtime>) -> Self {
        Self {
            connection,
            result_set_id,
            run_time,
        }
    }
}

impl ResultSetControl for NetworkResultSet {
    fn get_metadata(&self) -> Result<Metadata, anyhow::Error> {
        let mut client = ResultSetServiceClient::new(self.connection.get_channel());
        let request = Request::new(ResultSetGetMetadataRequest {
            id: self.result_set_id,
        });
        let _ = self
            .run_time
            .block_on(client.get_metadata(request))
            .unwrap()
            .into_inner();
        Ok(Metadata::Network(NetworkMetadata::new(
            self.connection.clone(),
            self.result_set_id,
            self.run_time.clone(),
        )))
    }

    fn next(&mut self) -> Result<bool, anyhow::Error> {
        let mut client = ResultSetServiceClient::new(self.connection.get_channel());
        let request = Request::new(ResultSetNextRequest {
            id: self.result_set_id,
        });
        let response = self
            .run_time
            .block_on(client.next(request))
            .unwrap()
            .into_inner();
        Ok(response.has_next)
    }

    fn previous(&mut self) -> Result<bool, anyhow::Error> {
        let mut client = ResultSetServiceClient::new(self.connection.get_channel());
        let request = Request::new(ResultSetPreviousRequest {
            id: self.result_set_id,
        });
        let response = self
            .run_time
            .block_on(client.previous(request))?
            .into_inner();
        Ok(response.has_prev)
    }

    fn before_first(&mut self) -> Result<(), anyhow::Error> {
        let mut client = ResultSetServiceClient::new(self.connection.get_channel());
        let request = Request::new(ResultSetBeforeFirstRequest {
            id: self.result_set_id,
        });
        self.run_time.block_on(client.before_first(request))?;
        Ok(())
    }

    fn after_last(&mut self) -> Result<(), anyhow::Error> {
        let mut client = ResultSetServiceClient::new(self.connection.get_channel());
        let request = Request::new(ResultSetAfterLastRequest {
            id: self.result_set_id,
        });
        self.run_time.block_on(client.after_last(request))?;
        Ok(())
    }

    fn absolute(&mut self, n: usize) -> Result<bool, anyhow::Error> {
        let mut client = ResultSetServiceClient::new(self.connection.get_channel());
        let request = Request::new(ResultSetAbsoluteRequest {
            id: self.result_set_id,
            n: n as u64,
        });
        let response = self
            .run_time
            .block_on(client.absolute(request))?
            .into_inner();
        Ok(response.has_row)
    }

    fn get_i32(&mut self, column_name: &str) -> Result<Option<i32>, anyhow::Error> {
        let mut client = ResultSetServiceClient::new(self.connection.get_channel());
        let request = Request::new(ResultSetGetI32Request {
            id: self.result_set_id,
            column_name: column_name.to_string(),
        });
        let response = self
            .run_time
            .block_on(client.get_i32(request))
            .unwrap()
            .into_inner();
        if response.was_null {
            Ok(None)
        } else {
            Ok(Some(response.value))
        }
    }

    fn get_string(&mut self, column_name: &str) -> Result<Option<String>, anyhow::Error> {
        let mut client = ResultSetServiceClient::new(self.connection.get_channel());
        let request = Request::new(ResultSetGetStringRequest {
            id: self.result_set_id,
            column_name: column_name.to_string(),
        });
        let response = self
            .run_time
            .block_on(client.get_string(request))
            .unwrap()
            .into_inner();
        if response.was_null {
            Ok(None)
        } else {
            Ok(Some(response.value))
        }
    }

    fn close(&mut self) -> Result<(), anyhow::Error> {
        let mut client = ResultSetServiceClient::new(self.connection.get_channel());
        let request = Request::new(ResultSetCloseRequest {
            id: self.result_set_id,
        });
        self.run_time.block_on(client.close(request))?;
        Ok(())
    }
}
