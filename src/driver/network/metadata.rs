use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::runtime::Runtime;
use tonic::{Request, Response, Status};

use crate::{
    driver::{embedded::EmbeddedMetadata, MetadataControl},
    proto::simpledb::{
        metadata_service_client::MetadataServiceClient, metadata_service_server::MetadataService,
        MetadataGetColumnCountRequest, MetadataGetColumnCountResponse,
        MetadataGetColumnDisplaySizeRequest, MetadataGetColumnDisplaySizeResponse,
        MetadataGetColumnNameRequest, MetadataGetColumnNameResponse, MetadataGetColumnTypeRequest,
        MetadataGetColumnTypeResponse,
    },
    record::field::Type,
};

use super::connection::NetworkConnection;

pub struct RemoteMetadata {
    embedded_metadata_dict: Arc<Mutex<HashMap<u64, EmbeddedMetadata>>>,
}

impl RemoteMetadata {
    pub fn new(embedded_metadata_dict: Arc<Mutex<HashMap<u64, EmbeddedMetadata>>>) -> Self {
        RemoteMetadata {
            embedded_metadata_dict,
        }
    }
}

#[tonic::async_trait]
impl MetadataService for RemoteMetadata {
    async fn get_column_count(
        &self,
        request: Request<MetadataGetColumnCountRequest>,
    ) -> Result<Response<MetadataGetColumnCountResponse>, Status> {
        let result_set_id = request.into_inner().id;
        let mut lock = self.embedded_metadata_dict.lock().unwrap();
        let metadata = lock
            .get_mut(&result_set_id)
            .expect(&format!("Unknown result_set_id: {}", result_set_id));
        let column_count = metadata.get_column_count().unwrap() as u64;
        Ok(Response::new(MetadataGetColumnCountResponse {
            column_count,
        }))
    }

    async fn get_column_name(
        &self,
        request: Request<MetadataGetColumnNameRequest>,
    ) -> Result<Response<MetadataGetColumnNameResponse>, Status> {
        let request = request.into_inner();
        let result_set_id = request.id;
        let index = request.index as usize;
        let mut lock = self.embedded_metadata_dict.lock().unwrap();
        let metadata = lock
            .get_mut(&result_set_id)
            .expect(&format!("Unknown result_set_id: {}", result_set_id));
        let column_name = metadata.get_column_name(index).unwrap();
        Ok(Response::new(MetadataGetColumnNameResponse { column_name }))
    }

    async fn get_column_display_size(
        &self,
        request: Request<MetadataGetColumnDisplaySizeRequest>,
    ) -> Result<Response<MetadataGetColumnDisplaySizeResponse>, Status> {
        let request = request.get_ref();
        let result_set_id = request.id;
        let index = request.index as usize;
        let mut lock = self.embedded_metadata_dict.lock().unwrap();
        let metadata = lock
            .get_mut(&result_set_id)
            .expect(&format!("Unknown result_set_id: {}", result_set_id));
        let size = metadata.get_column_display_size(index).unwrap() as u64;
        Ok(Response::new(MetadataGetColumnDisplaySizeResponse { size }))
    }

    async fn get_column_type(
        &self,
        request: Request<MetadataGetColumnTypeRequest>,
    ) -> Result<Response<MetadataGetColumnTypeResponse>, Status> {
        let request = request.into_inner();
        let result_set_id = request.id;
        let index = request.index as usize;
        let mut lock = self.embedded_metadata_dict.lock().unwrap();
        let metadata = lock
            .get_mut(&result_set_id)
            .expect(&format!("Unknown result_set_id: {}", result_set_id));

        let column_type = metadata.get_column_type(index).unwrap();
        let type_code = match column_type {
            Type::I32 => 0,
            Type::String => 1,
        };
        Ok(Response::new(MetadataGetColumnTypeResponse { type_code }))
    }
}

pub struct NetworkMetadata {
    connection: NetworkConnection,
    metadata_id: u64,
    run_time: Arc<Runtime>,
}

impl NetworkMetadata {
    pub fn new(connection: NetworkConnection, metadata_id: u64, run_time: Arc<Runtime>) -> Self {
        NetworkMetadata {
            connection,
            metadata_id,
            run_time,
        }
    }
}

impl MetadataControl for NetworkMetadata {
    fn get_column_count(&self) -> Result<usize, anyhow::Error> {
        let mut client = MetadataServiceClient::new(self.connection.get_channel());
        let response = self
            .run_time
            .block_on(client.get_column_count(MetadataGetColumnCountRequest {
                id: self.metadata_id,
            }))
            .unwrap();
        Ok(response.into_inner().column_count as usize)
    }

    fn get_column_name(&self, index: usize) -> Result<String, anyhow::Error> {
        let mut client = MetadataServiceClient::new(self.connection.get_channel());
        let response = self
            .run_time
            .block_on(client.get_column_name(MetadataGetColumnNameRequest {
                id: self.metadata_id,
                index: index as u64,
            }))
            .unwrap();
        Ok(response.into_inner().column_name)
    }

    fn get_column_display_size(&self, index: usize) -> Result<usize, anyhow::Error> {
        let mut client = MetadataServiceClient::new(self.connection.get_channel());
        let response = self
            .run_time
            .block_on(
                client.get_column_display_size(MetadataGetColumnDisplaySizeRequest {
                    id: self.metadata_id,
                    index: index as u64,
                }),
            )
            .unwrap();
        Ok(response.into_inner().size as usize)
    }

    fn get_column_type(&self, index: usize) -> Result<Type, anyhow::Error> {
        let mut client = MetadataServiceClient::new(self.connection.get_channel());
        let response = self
            .run_time
            .block_on(client.get_column_type(MetadataGetColumnTypeRequest {
                id: self.metadata_id,
                index: index as u64,
            }))
            .unwrap();

        let type_code = response.into_inner().type_code;
        match type_code {
            0 => Ok(Type::I32),
            1 => Ok(Type::String),
            _ => panic!("Unknown type code: {}", type_code),
        }
    }
}
