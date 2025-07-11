use std::{
    cmp::max,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::{
    config,
    db::SimpleDB,
    errors::ExecutionError,
    plan::{Plan, PlanControl},
    planner::Planner,
    record::{
        field::{Spec, Type},
        schema::Schema,
    },
    scan::{Scan, ScanControl},
    tx::transaction::Transaction,
};

use super::{
    Connection, ConnectionControl, DriverControl, Metadata, MetadataControl, ResultSet,
    ResultSetControl, Statement, StatementControl,
};

const MAX_VARCHAR_DISPLAY_SIZE: usize = 30;

pub struct EmbeddedMetadata {
    schema: Schema,
}

impl MetadataControl for EmbeddedMetadata {
    fn get_column_count(&self) -> Result<usize, anyhow::Error> {
        Ok(self.schema.get_fields().len())
    }

    fn get_column_name(&self, index: usize) -> Result<String, anyhow::Error> {
        Ok(self.schema.get_fields()[index].clone())
    }

    fn get_column_display_size(&self, index: usize) -> Result<usize, anyhow::Error> {
        let name = self.get_column_name(index)?;
        let size = match self.schema.get_field_spec(&name) {
            Spec::I32 => 12,
            Spec::VarChar(max_len) => std::cmp::min(max_len, MAX_VARCHAR_DISPLAY_SIZE),
        };
        Ok(max(size, name.len()))
    }

    fn get_column_type(&self, index: usize) -> Result<Type, anyhow::Error> {
        let name = self.get_column_name(index)?;
        Ok(self.schema.get_field_type(&name))
    }
}

pub struct EmbeddedResultSet {
    connection: Arc<Mutex<EmbeddedConnectionImpl>>,
    scan: Option<Scan>,
    schema: Schema,
}

impl EmbeddedResultSet {
    fn new(
        mut plan: Plan,
        connection: Arc<Mutex<EmbeddedConnectionImpl>>,
    ) -> Result<Self, ExecutionError> {
        let scan = Some(plan.open(connection.lock().unwrap().get_transaction())?);
        Ok(EmbeddedResultSet {
            connection,
            scan,
            schema: plan.schema().clone(),
        })
    }
}

impl ResultSetControl for EmbeddedResultSet {
    fn get_metadata(&self) -> Result<Metadata, anyhow::Error> {
        Ok(Metadata::Embedded(EmbeddedMetadata {
            schema: self.schema.clone(),
        }))
    }

    fn next(&mut self) -> Result<bool, anyhow::Error> {
        Ok(self.scan.as_mut().unwrap().next()?)
    }

    fn previous(&mut self) -> Result<bool, anyhow::Error> {
        Ok(self.scan.as_mut().unwrap().previous()?)
    }

    fn before_first(&mut self) -> Result<(), anyhow::Error> {
        self.scan.as_mut().unwrap().before_first()?;
        Ok(())
    }

    fn after_last(&mut self) -> Result<(), anyhow::Error> {
        self.scan.as_mut().unwrap().after_last()?;
        Ok(())
    }

    fn absolute(&mut self, n: usize) -> Result<bool, anyhow::Error> {
        self.before_first()?;
        for _ in 0..=n {
            if !self.next()? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn get_i32(&mut self, column_name: &str) -> Result<Option<i32>, anyhow::Error> {
        self.scan
            .as_mut()
            .unwrap()
            .get_i32(column_name)
            .map_err(|e| anyhow::anyhow!("Error getting i32 from column '{}': {}", column_name, e))
    }

    fn get_string(&mut self, column_name: &str) -> Result<Option<String>, anyhow::Error> {
        self.scan
            .as_mut()
            .unwrap()
            .get_string(column_name)
            .map_err(|e| {
                anyhow::anyhow!("Error getting string from column '{}': {}", column_name, e)
            })
    }

    fn close(&mut self) -> Result<(), anyhow::Error> {
        self.scan = None;
        Ok(self.connection.lock().unwrap().commit()?)
    }
}

pub struct EmbeddedStatement {
    connection: Arc<Mutex<EmbeddedConnectionImpl>>,
    planner: Arc<Mutex<Planner>>,
}

impl EmbeddedStatement {
    fn new(connection: Arc<Mutex<EmbeddedConnectionImpl>>) -> Result<Self, ExecutionError> {
        let planner = connection.lock().unwrap().db.planner.clone();
        Ok(Self {
            connection,
            planner,
        })
    }
}

impl StatementControl for EmbeddedStatement {
    fn execute_query(&mut self, command: &str) -> Result<ResultSet, anyhow::Error> {
        let tx = self.connection.lock().unwrap().get_transaction();
        let plan = self
            .planner
            .lock()
            .unwrap()
            .create_query_plan(command, tx)?;
        let result_set: EmbeddedResultSet = EmbeddedResultSet::new(plan, self.connection.clone())?;
        return Ok(ResultSet::Embedded(result_set));
    }

    fn execute_update(&mut self, command: &str) -> Result<usize, anyhow::Error> {
        let tx = self.connection.lock().unwrap().get_transaction();
        let num_updated = self.planner.lock().unwrap().execute_update(command, tx)?;
        self.connection.lock().unwrap().commit()?;
        Ok(num_updated)
    }
}

pub struct EmbeddedConnectionImpl {
    db: SimpleDB,
    current_tx: Arc<Mutex<Transaction>>,
}

impl EmbeddedConnectionImpl {
    fn close(&self) -> Result<(), ExecutionError> {
        self.current_tx.lock().unwrap().commit()?;
        Ok(())
    }

    fn print_file_access_stats(&self) {
        if !config::is_verbose() {
            return;
        }
        let mut file_manager = self.db.file_manager.lock().unwrap();
        let tx_write_count = file_manager.file_access_stats.write_count;
        let tx_read_count = file_manager.file_access_stats.read_count;
        file_manager.file_access_stats.reset();
        eprintln!(
            "Transaction read count: {}, write count: {}",
            tx_read_count, tx_write_count
        );
    }

    fn commit(&mut self) -> Result<(), ExecutionError> {
        self.current_tx.lock().unwrap().commit()?;
        let new_tx = Arc::new(Mutex::new(self.db.new_transaction()?));
        self.current_tx = new_tx;
        self.print_file_access_stats();
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), ExecutionError> {
        self.current_tx.lock().unwrap().rollback()?;
        let new_tx = Arc::new(Mutex::new(self.db.new_transaction()?));
        self.current_tx = new_tx;
        self.print_file_access_stats();
        Ok(())
    }

    fn get_transaction(&self) -> Arc<Mutex<Transaction>> {
        self.current_tx.clone()
    }
}

pub struct EmbeddedConnection {
    connection: Arc<Mutex<EmbeddedConnectionImpl>>,
}

impl EmbeddedConnection {
    fn new(db: SimpleDB) -> Result<Self, ExecutionError> {
        {
            let mut file_manager = db.file_manager.lock().unwrap();
            file_manager.file_access_stats.reset();
        }
        let current_tx = Arc::new(Mutex::new(db.new_transaction()?));
        let connection = EmbeddedConnectionImpl { db, current_tx };
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }
}

impl ConnectionControl for EmbeddedConnection {
    fn create_statement(&self) -> Result<Statement, anyhow::Error> {
        Ok(Statement::Embedded(EmbeddedStatement::new(
            self.connection.clone(),
        )?))
    }

    fn close(&mut self) -> Result<(), anyhow::Error> {
        Ok(self.connection.lock().unwrap().close()?)
    }

    fn commit(&mut self) -> Result<(), anyhow::Error> {
        Ok(self.connection.lock().unwrap().commit()?)
    }

    fn rollback(&mut self) -> Result<(), anyhow::Error> {
        Ok(self.connection.lock().unwrap().rollback()?)
    }
}

// TODO: Locate these constants in the appropriate module
const DEFAULT_BLOCK_SIZE: usize = 1024;
const DEFAULT_NUM_BUFFERS: usize = 256;

pub struct EmbeddedDriver {}

impl EmbeddedDriver {
    pub fn new() -> Self {
        Self {}
    }
}

impl DriverControl for EmbeddedDriver {
    fn connect(&self, db_url: &str) -> Result<(String, Connection), anyhow::Error> {
        let db_name = db_url.trim().to_string();
        let db_directory = PathBuf::from(&db_name);
        let db = SimpleDB::new(db_directory, DEFAULT_BLOCK_SIZE, DEFAULT_NUM_BUFFERS)?;
        Ok((db_name, Connection::Embedded(EmbeddedConnection::new(db)?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_driver_basic_flow() -> Result<(), anyhow::Error> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let db_name = temp_dir.to_string_lossy().to_string();

        let driver = EmbeddedDriver::new();
        let (db_name, mut connection) = driver.connect(&db_name)?;
        assert_eq!(db_name, temp_dir.to_string_lossy());

        let mut statement = connection.create_statement()?;
        statement.execute_update("create table test (A I32, B VARCHAR(20))")?;
        statement.execute_update("insert into test (A, B) values (1, 'a')")?;
        statement.execute_update("insert into test (A, B) values (2, 'b')")?;

        let mut result_set = statement.execute_query("select B from test")?;
        let metadata = result_set.get_metadata()?;
        assert_eq!(metadata.get_column_count()?, 1);
        assert_eq!(metadata.get_column_name(0)?, "B");

        assert!(result_set.next()?);
        assert_eq!(result_set.get_string("B")?, Some("a".to_string()));
        assert!(result_set.next()?);
        assert_eq!(result_set.get_string("B")?, Some("b".to_string()));
        assert!(!result_set.next()?);

        result_set.after_last()?;
        assert!(result_set.previous()?);
        assert_eq!(result_set.get_string("B")?, Some("b".to_string()));
        assert!(result_set.previous()?);
        assert_eq!(result_set.get_string("B")?, Some("a".to_string()));
        assert!(!result_set.previous()?);
        result_set.before_first()?;
        assert!(result_set.absolute(0)?);
        assert_eq!(result_set.get_string("B")?, Some("a".to_string()));
        assert!(result_set.absolute(1)?);
        assert_eq!(result_set.get_string("B")?, Some("b".to_string()));
        assert!(!result_set.absolute(2)?);
        result_set.close()?;

        connection.close()?;
        Ok(())
    }

    #[test]
    fn test_embedded_driver_null_handling() -> Result<(), anyhow::Error> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("dir_null");
        let db_name = temp_dir.to_string_lossy();

        let driver = EmbeddedDriver::new();
        let (_db_name, connection) = driver.connect(&db_name)?;

        let mut statement = connection.create_statement()?;
        statement.execute_update("create table test (A I32, B VARCHAR(20))")?;
        statement.execute_update("insert into test (A, B) values (1, 'a')")?;
        statement.execute_update("insert into test (A, B) values (2, NULL)")?;

        let mut result_set = statement.execute_query("select B from test")?;
        assert!(result_set.next()?);
        assert_eq!(result_set.get_string("B")?, Some("a".to_string()));
        assert!(result_set.next()?);
        assert_eq!(result_set.get_string("B")?, None);
        Ok(())
    }
}
