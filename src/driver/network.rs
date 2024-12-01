// use std::sync::Arc;
// use std::{collections::HashMap, sync::Mutex};

// use tonic::transport::Channel;
// use tonic::{Request, Response, Result, Status};

// use super::embedded::{self, EmbeddedResultSet};
// use super::{ConnectionAdaptor, Driver, Metadata};
// use super::{MetadataControl, ResultSetControl};

// use crate::proto::simpledb::connection_service_server::ConnectionService;
// use crate::proto::simpledb::metadata_service_client::MetadataServiceClient;
// use crate::proto::simpledb::result_set_service_server::ResultSetService;
// use crate::proto::simpledb::statement_service_server::StatementService;
// use crate::proto::simpledb::{
//     metadata_service_server::MetadataService, MetadataGetColumnCountRequest,
//     MetadataGetColumnCountResponse, MetadataGetColumnDisplaySizeRequest,
//     MetadataGetColumnDisplaySizeResponse, MetadataGetColumnNameRequest,
//     MetadataGetColumnNameResponse, MetadataGetColumnTypeRequest, MetadataGetColumnTypeResponse,
// };
// use crate::proto::simpledb::{
//     ConnectionCloseRequest, ConnectionCloseResponse, ConnectionCommitRequest,
//     ConnectionCommitResponse, ConnectionCreateStatementRequest, ConnectionCreateStatementResponse,
//     ConnectionRollbackRequest, ConnectionRollbackResponse, ResultSetCloseRequest,
//     ResultSetCloseResponse, ResultSetGetI32Request, ResultSetGetI32Response,
//     ResultSetGetMetadataRequest, ResultSetGetMetadataResponse, ResultSetGetStringRequest,
//     ResultSetGetStringResponse, ResultSetNextRequest, ResultSetNextResponse,
//     StatementExecuteQueryRequest, StatementExecuteQueryResponse, StatementExecuteUpdateRequest,
//     StatementExecuteUpdateResponse,
// };
// use crate::record::field::Type;

// struct RemoteMetadata {
//     embedded_metadata_dict: Mutex<HashMap<u64, Metadata>>,
// }

// #[tonic::async_trait]
// impl MetadataService for RemoteMetadata {
//     async fn get_column_count(
//         &self,
//         request: Request<MetadataGetColumnCountRequest>,
//     ) -> Result<Response<MetadataGetColumnCountResponse>, Status> {
//         let result_set_id = request.into_inner().id;
//         let mut lock = self.embedded_metadata_dict.lock().unwrap();
//         let metadata = lock
//             .get_mut(&result_set_id)
//             .expect(&format!("Unknown result_set_id: {}", result_set_id));
//         let column_count = metadata.get_column_count() as u64;
//         Ok(Response::new(MetadataGetColumnCountResponse {
//             column_count,
//         }))
//     }

//     async fn get_column_name(
//         &self,
//         request: Request<MetadataGetColumnNameRequest>,
//     ) -> Result<Response<MetadataGetColumnNameResponse>, Status> {
//         let request = request.into_inner();
//         let result_set_id = request.id;
//         let index = request.index as usize;
//         let mut lock = self.embedded_metadata_dict.lock().unwrap();
//         let metadata = lock
//             .get_mut(&result_set_id)
//             .expect(&format!("Unknown result_set_id: {}", result_set_id));
//         let column_name = metadata.get_column_name(index);
//         Ok(Response::new(MetadataGetColumnNameResponse { column_name }))
//     }

//     async fn get_column_display_size(
//         &self,
//         request: Request<MetadataGetColumnDisplaySizeRequest>,
//     ) -> Result<Response<MetadataGetColumnDisplaySizeResponse>, Status> {
//         let request = request.get_ref();
//         let result_set_id = request.id;
//         let index = request.index as usize;
//         let mut lock = self.embedded_metadata_dict.lock().unwrap();
//         let metadata = lock
//             .get_mut(&result_set_id)
//             .expect(&format!("Unknown result_set_id: {}", result_set_id));
//         let size = metadata.get_column_display_size(index) as u64;
//         Ok(Response::new(MetadataGetColumnDisplaySizeResponse { size }))
//     }

//     async fn get_column_type(
//         &self,
//         request: Request<MetadataGetColumnTypeRequest>,
//     ) -> Result<Response<MetadataGetColumnTypeResponse>, Status> {
//         let request = request.into_inner();
//         let result_set_id = request.id;
//         let index = request.index as usize;
//         let mut lock = self.embedded_metadata_dict.lock().unwrap();
//         let metadata = lock
//             .get_mut(&result_set_id)
//             .expect(&format!("Unknown result_set_id: {}", result_set_id));

//         let column_type = metadata.get_column_type(index);
//         let type_code = match column_type {
//             Type::I32 => 0,
//             Type::String => 1,
//         };
//         Ok(Response::new(MetadataGetColumnTypeResponse { type_code }))
//     }
// }

// pub struct NetworkMetadata {
//     remote_metadata: Arc<Mutex<MetadataServiceClient<Channel>>>,
//     metadata_id: u64,
// }

// impl MetadataControl for NetworkMetadata {
//     fn get_column_count(&self) -> usize {
//         let client = self.remote_metadata.lock().unwrap();

//         // client.get_column_count(MetadataGetColum nCountRequest {
//         //     id: self.metadata_id,
//         // });
//         0
//     }

//     fn get_column_name(&self, index: usize) -> String {s
//         todo!()
//     }

//     fn get_column_display_size(&self, index: usize) -> usize {
//         todo!()
//     }

//     fn get_column_type(&self, index: usize) -> Type {
//         todo!()
//     }
// }

// struct RemoteResultSet {
//     embedded_result_set_dict: Mutex<HashMap<u64, EmbeddedResultSet>>,
//     remote_metadata: Arc<Mutex<RemoteMetadata>>,
// }

// #[tonic::async_trait]
// impl ResultSetService for RemoteResultSet {
//     async fn get_metadata(
//         &self,
//         request: Request<ResultSetGetMetadataRequest>,
//     ) -> Result<Response<ResultSetGetMetadataResponse>, Status> {
//         let request = request.into_inner();
//         let result_set_id = request.id;
//         let lock = self.embedded_result_set_dict.lock().unwrap();
//         let embedded_result_set = lock
//             .get(&result_set_id)
//             .expect(&format!("Unknown result_set_id: {}", result_set_id));

//         let embedded_metadata = embedded_result_set.get_metadata();

//         let remote_metadata = self.remote_metadata.lock().unwrap();
//         remote_metadata
//             .embedded_metadata_dict
//             .lock()
//             .unwrap()
//             .insert(result_set_id, embedded_metadata);

//         Ok(Response::new(ResultSetGetMetadataResponse {
//             metadata_id: result_set_id,
//         }))
//     }

//     async fn next(
//         &self,
//         request: Request<ResultSetNextRequest>,
//     ) -> Result<Response<ResultSetNextResponse>, Status> {
//         let remote_metadata = self.remote_metadata.lock().unwrap();
//         let request = request.into_inner();
//         let result_set_id = request.id;
//         let mut lock = self.embedded_result_set_dict.lock().unwrap();
//         let embedded_result_set = lock
//             .get_mut(&result_set_id)
//             .expect(&format!("Unknown result_set_id: {}", result_set_id));
//         let has_next = embedded_result_set.next()?;
//         Ok(Response::new(ResultSetNextResponse { has_next }))
//     }

//     async fn get_i32(
//         &self,
//         request: Request<ResultSetGetI32Request>,
//     ) -> Result<Response<ResultSetGetI32Response>, Status> {
//         let request = request.into_inner();
//         let result_set_id = request.id;
//         let column_name = request.column_name;
//         let mut lock = self.embedded_result_set_dict.lock().unwrap();
//         let embedded_result_set = lock
//             .get_mut(&result_set_id)
//             .expect(&format!("Unknown result_set_id: {}", result_set_id));
//         let value = embedded_result_set.get_i32(&column_name)?;
//         Ok(Response::new(ResultSetGetI32Response { value }))
//     }

//     async fn get_string(
//         &self,
//         request: Request<ResultSetGetStringRequest>,
//     ) -> Result<Response<ResultSetGetStringResponse>, Status> {
//         let request = request.into_inner();
//         let result_set_id = request.id;
//         let column_name = request.column_name;
//         let mut lock = self.embedded_result_set_dict.lock().unwrap();
//         let embedded_result_set = lock
//             .get_mut(&result_set_id)
//             .expect(&format!("Unknown result_set_id: {}", result_set_id));
//         let value = embedded_result_set.get_string(&column_name)?;
//         Ok(Response::new(ResultSetGetStringResponse { value }))
//     }

//     async fn close(
//         &self,
//         request: Request<ResultSetCloseRequest>,
//     ) -> Result<Response<ResultSetCloseResponse>, Status> {
//         // let request = request.into_inner();
//         // let result_set_id = request.id;
//         // let mut lock = self.embedded_result_set_dict.lock().unwrap();

//         Ok(Response::new(ResultSetCloseResponse {}))
//     }
// }

// struct RemoteStatement {}

// #[tonic::async_trait]
// impl StatementService for RemoteStatement {
//     async fn execute_query(
//         &self,
//         request: Request<StatementExecuteQueryRequest>,
//     ) -> Result<Response<StatementExecuteQueryResponse>, Status> {
//         todo!()
//     }
//     async fn execute_update(
//         &self,
//         request: Request<StatementExecuteUpdateRequest>,
//     ) -> Result<Response<StatementExecuteUpdateResponse>, Status> {
//         todo!()
//     }
// }

// struct RemoteConnection {}

// #[tonic::async_trait]
// impl ConnectionService for RemoteConnection {
//     async fn create_statement(
//         &self,
//         request: Request<ConnectionCreateStatementRequest>,
//     ) -> Result<Response<ConnectionCreateStatementResponse>, Status> {
//         todo!()
//     }

//     async fn close(
//         &self,
//         request: Request<ConnectionCloseRequest>,
//     ) -> Result<Response<ConnectionCloseResponse>, Status> {
//         todo!()
//     }

//     async fn commit(
//         &self,
//         request: Request<ConnectionCommitRequest>,
//     ) -> Result<Response<ConnectionCommitResponse>, Status> {
//         todo!()
//     }

//     async fn rollback(
//         &self,
//         request: Request<ConnectionRollbackRequest>,
//     ) -> Result<Response<ConnectionRollbackResponse>, Status> {
//         todo!()
//     }
// }

// pub struct NetworkDriver {}

// impl NetworkDriver {
//     pub fn new() -> Self {
//         NetworkDriver {}
//     }
// }

// impl Driver for NetworkDriver {
//     fn connect(&self, db_url: &str) -> Result<(String, Box<dyn ConnectionAdaptor>), anyhow::Error> {
//         // let mut client = HogeFugaClient::connect("http://[::1]:50051").await?;

//         // let request = tonic::Request::new(HelloRequest {
//         //     name: "Tonic".into(),
//         // });

//         // let response = client.say_hello(request).await?;
//         todo!()
//     }
// }
