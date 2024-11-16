use std::{
    cell::RefCell,
    cmp::max,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
};

use crate::{
    db::SimpleDB,
    plan::{planner::Planner, Plan},
    record::{field::Type, schema::Schema},
    scan::Scan,
    tx::transaction::Transaction,
};

use super::{ConnectionAdaptor, Metadata, ResultSet, Statement};

struct EmbeddedMetadata {
    schema: Schema,
}

impl Metadata for EmbeddedMetadata {
    fn get_column_count(&self) -> usize {
        self.schema.get_fields().len()
    }

    fn get_column_name(&self, index: usize) -> String {
        self.schema.get_fields()[index].clone()
    }

    fn get_column_display_size(&self, index: usize) -> usize {
        let name = self.get_column_name(index);
        let size = match self.get_column_type(index) {
            Type::I32 => 12,
            Type::String => name.len(),
        };
        max(size, name.len())
    }

    fn get_column_type(&self, index: usize) -> Type {
        let name = self.get_column_name(index);
        self.schema.get_field_type(&name)
    }
}

struct EmbeddedResultSet {
    connection: Rc<RefCell<EmbeddedConnectionImpl>>,
    scan: Box<dyn Scan>,
    schema: Schema,
}

impl EmbeddedResultSet {
    fn new(
        mut plan: Box<dyn Plan>,
        connection: Rc<RefCell<EmbeddedConnectionImpl>>,
    ) -> Result<Self, anyhow::Error> {
        let scan = plan.open(connection.borrow().get_transaction())?;
        Ok(EmbeddedResultSet {
            connection,
            scan,
            schema: plan.schema().clone(),
        })
    }
}

impl ResultSet for EmbeddedResultSet {
    fn get_metadata(&self) -> Box<dyn Metadata> {
        Box::new(EmbeddedMetadata {
            schema: self.schema.clone(),
        })
    }

    fn next(&mut self) -> Result<bool, anyhow::Error> {
        self.scan.next()
    }

    fn get_i32(&mut self, column_name: &str) -> Result<i32, anyhow::Error> {
        self.scan.get_i32(column_name)
    }

    fn get_string(&mut self, column_name: &str) -> Result<String, anyhow::Error> {
        self.scan.get_string(column_name)
    }

    fn close(&mut self) -> Result<(), anyhow::Error> {
        // self.scan.close()?;
        self.connection.borrow_mut().commit()
    }
}

struct EmbeddedStatement {
    connection: Rc<RefCell<EmbeddedConnectionImpl>>,
    planner: Arc<Mutex<Planner>>,
}

impl EmbeddedStatement {
    fn new(connection: Rc<RefCell<EmbeddedConnectionImpl>>) -> Result<Self, anyhow::Error> {
        let planner = connection.borrow().db.planner.clone();
        Ok(Self {
            connection,
            planner,
        })
    }
}

impl Statement for EmbeddedStatement {
    fn execute_query(&mut self, command: &str) -> Result<Box<dyn ResultSet>, anyhow::Error> {
        let tx = self.connection.borrow().get_transaction();
        let plan = self
            .planner
            .lock()
            .unwrap()
            .create_query_plan(command, tx)?;
        let result_set: EmbeddedResultSet = EmbeddedResultSet::new(plan, self.connection.clone())?;
        return Ok(Box::new(result_set));
    }

    fn execute_update(&mut self, command: &str) -> Result<usize, anyhow::Error> {
        let tx = self.connection.borrow().get_transaction();
        let num_updated = self.planner.lock().unwrap().execute_update(command, tx)?;
        self.connection.borrow_mut().commit()?;
        Ok(num_updated)
    }
}

struct EmbeddedConnectionImpl {
    db: SimpleDB,
    current_tx: Arc<Mutex<Transaction>>,
}

impl EmbeddedConnectionImpl {
    fn close(&self) -> Result<(), anyhow::Error> {
        self.current_tx.lock().unwrap().commit()?;
        Ok(())
    }

    fn commit(&mut self) -> Result<(), anyhow::Error> {
        self.current_tx.lock().unwrap().commit()?;
        let new_tx = Arc::new(Mutex::new(self.db.new_transaction()?));
        self.current_tx = new_tx;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), anyhow::Error> {
        self.current_tx.lock().unwrap().rollback()?;
        let new_tx = Arc::new(Mutex::new(self.db.new_transaction()?));
        self.current_tx = new_tx;
        Ok(())
    }

    fn get_transaction(&self) -> Arc<Mutex<Transaction>> {
        self.current_tx.clone()
    }
}

struct EmbeddedConnection {
    connection: Rc<RefCell<EmbeddedConnectionImpl>>,
}

impl EmbeddedConnection {
    fn new(db: SimpleDB) -> Result<Self, anyhow::Error> {
        let current_tx = Arc::new(Mutex::new(db.new_transaction()?));
        let connection = EmbeddedConnectionImpl { db, current_tx };
        Ok(Self {
            connection: Rc::new(RefCell::new(connection)),
        })
    }
}

impl ConnectionAdaptor for EmbeddedConnection {
    fn create_statement(&self) -> Result<Box<dyn Statement>, anyhow::Error> {
        Ok(Box::new(EmbeddedStatement::new(self.connection.clone())?))
    }

    fn close(&mut self) -> Result<(), anyhow::Error> {
        self.connection.borrow().close()
    }

    fn commit(&mut self) -> Result<(), anyhow::Error> {
        self.connection.borrow_mut().commit()
    }

    fn rollback(&mut self) -> Result<(), anyhow::Error> {
        self.connection.borrow_mut().rollback()
    }
}

// TODO: Locate these constants in the appropriate module
const DEFAULT_BLOCK_SIZE: usize = 256;
const DEFAULT_NUM_BUFFERS: usize = 32;

pub struct EmbeddedDriver {}

impl EmbeddedDriver {
    pub fn new() -> Self {
        Self {}
    }

    pub fn connect(
        &self,
        db_url: &str,
    ) -> Result<(String, Box<dyn ConnectionAdaptor>), anyhow::Error> {
        let db_name = db_url.replace("jdbc:simpledb:", "").trim().to_string();
        let db_directory = PathBuf::from(&db_name);
        let db = SimpleDB::new(db_directory, DEFAULT_BLOCK_SIZE, DEFAULT_NUM_BUFFERS)?;
        Ok((db_name, Box::new(EmbeddedConnection::new(db)?)))
    }
}
