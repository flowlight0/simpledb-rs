use crate::driver::{Connection, Driver};

pub struct NetworkDriver {}

impl NetworkDriver {
    pub fn new() -> Self {
        NetworkDriver {}
    }
}

impl Driver for NetworkDriver {
    fn connect(&self, db_url: &str) -> Result<(String, Connection), anyhow::Error> {
        // let mut client = HogeFugaClient::connect("http://[::1]:50051").await?;

        // let request = tonic::Request::new(HelloRequest {
        //     name: "Tonic".into(),
        // });

        // let response = client.say_hello(request).await?;
        todo!()
    }
}
