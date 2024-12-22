use std::sync::{Arc, Mutex};

use lalrpop_util::{lexer::Token, ParseError};

use crate::{
    errors::{ExecutionError, QueryError},
    parser::grammar::{QueryParser, UpdateCommandParser},
    tx::transaction::Transaction,
};

use super::{Plan, QueryPlanner, UpdatePlanner};

pub struct Planner {
    query_planner: Box<dyn QueryPlanner>,
    update_planner: Box<dyn UpdatePlanner>,
}

impl Planner {
    pub fn new(
        query_planner: Box<dyn QueryPlanner>,
        update_planner: Box<dyn UpdatePlanner>,
    ) -> Self {
        Self {
            query_planner,
            update_planner,
        }
    }

    pub fn create_query_plan(
        &self,
        query: &str,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<Box<dyn Plan>, ExecutionError> {
        // Discard the parse error because it requires static lifetime for the query
        let query_data = QueryParser::new()
            .parse(query)
            .map_err(|e: ParseError<usize, Token<'_>, &str>| QueryError::from(e))?;
        Ok(self.query_planner.create_plan(&query_data, tx)?)
    }

    pub fn execute_update(
        &self,
        update_command: &str,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<usize, ExecutionError> {
        // Discard the parse error because it requires static lifetime for the update_command
        let update_command = UpdateCommandParser::new()
            .parse(&update_command)
            .map_err(QueryError::from)?;
        Ok(self.update_planner.execute_update(&update_command, tx)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::SimpleDB;

    #[test]
    fn test_planner() -> Result<(), ExecutionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let num_buffers = 100;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let updated = db
            .planner
            .lock()
            .unwrap()
            .execute_update("CREATE TABLE table1 (A I32, B VARCHAR(20))", tx.clone())?;
        assert_eq!(updated, 0);

        for i in 0..10 {
            let updated = db.planner.lock().unwrap().execute_update(
                &format!("INSERT INTO table1 (A, B) VALUES ({}, '{}')", i % 2, i),
                tx.clone(),
            )?;
            assert_eq!(updated, 1);
        }

        let deleted = db
            .planner
            .lock()
            .unwrap()
            .execute_update("DELETE FROM table1 WHERE A = 0", tx.clone())?;
        assert_eq!(deleted, 5);

        let mut query = db
            .planner
            .lock()
            .unwrap()
            .create_query_plan("SELECT B FROM table1", tx.clone())?;
        let mut scan = query.open(tx.clone())?;

        for i in 0..5 {
            assert!(scan.next()?);
            assert_eq!(scan.get_string("B")?, (i * 2 + 1).to_string());
        }
        assert!(!scan.next()?);
        drop(scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }
}
