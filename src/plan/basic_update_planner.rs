use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    metadata::MetadataManager,
    parser::{
        predicate::{Expression, Predicate},
        statement::{CreateCommand, UpdateCommand},
    },
    record::{field::Value, schema::Schema},
    scan::ScanControl,
    tx::transaction::Transaction,
};

use super::{select_plan::SelectPlan, table_plan::TablePlan, Plan, UpdatePlanner};

pub struct BasicUpdatePlanner {
    metadata_manager: Arc<Mutex<MetadataManager>>,
}

impl BasicUpdatePlanner {
    pub fn new(metadata_manager: Arc<Mutex<MetadataManager>>) -> Self {
        Self { metadata_manager }
    }

    fn insert(
        &self,
        table_name: &str,
        fields: &Vec<String>,
        values: &Vec<Value>,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<usize, TransactionError> {
        let mut table_plan = TablePlan::new(tx.clone(), table_name, self.metadata_manager.clone())?;
        let mut table_scan = table_plan.open(tx)?;
        table_scan.insert()?;
        for (field, value) in fields.iter().zip(values.iter()) {
            table_scan.set_value(field, value)?;
        }
        Ok(1)
    }

    fn delete(
        &self,
        table_name: &str,
        predicate: &Option<Predicate>,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<usize, TransactionError> {
        let table_plan = TablePlan::new(tx.clone(), table_name, self.metadata_manager.clone())?;
        let mut plan: Box<dyn Plan> = if let Some(predicate) = predicate {
            Box::new(SelectPlan::new(Box::new(table_plan), predicate.clone()))
        } else {
            Box::new(table_plan)
        };
        let mut scan = plan.open(tx.clone())?;
        let mut count = 0;
        while scan.next()? {
            scan.delete()?;
            count += 1;
        }
        Ok(count)
    }

    fn modify(
        &self,
        table_name: &str,
        field_name: &str,
        expression: &Expression,
        predicate: &Option<Predicate>,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<usize, TransactionError> {
        let table_plan = TablePlan::new(tx.clone(), table_name, self.metadata_manager.clone())?;
        let mut plan: Box<dyn Plan> = if let Some(predicate) = predicate {
            Box::new(SelectPlan::new(Box::new(table_plan), predicate.clone()))
        } else {
            Box::new(table_plan)
        };

        let mut scan = plan.open(tx.clone())?;
        let mut count = 0;
        while scan.next()? {
            let new_value = expression.evaluate(&mut scan)?;
            scan.set_value(field_name, &new_value)?;
            count += 1;
        }
        Ok(count)
    }

    fn create(
        &self,
        create_command: &CreateCommand,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<usize, TransactionError> {
        match create_command {
            CreateCommand::Table(table_name, fields) => {
                let schema = Schema::create_from(&fields);
                let mut metadata_manager = self.metadata_manager.lock().unwrap();
                metadata_manager.create_table(&table_name, &schema, tx)?
            }
            _ => unimplemented!("Creation of indexes and views is not supported yet"),
        }
        Ok(0)
    }
}

impl UpdatePlanner for BasicUpdatePlanner {
    fn execute_update(
        &self,
        update_command: &UpdateCommand,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<usize, TransactionError> {
        match update_command {
            UpdateCommand::Insert(table_name, fields, values) => {
                self.insert(table_name, fields, values, tx)
            }
            UpdateCommand::Delete(table_name, predicate) => self.delete(table_name, predicate, tx),
            UpdateCommand::Modify(table_name, field_name, expression, predicate) => {
                self.modify(table_name, field_name, expression, predicate, tx)
            }
            UpdateCommand::Create(create_command) => self.create(create_command, tx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::SimpleDB;

    use crate::parser::predicate::{Expression, Predicate, Term};

    use crate::parser::statement::{FieldDefinition, QueryData};
    use crate::plan::basic_query_planner::BasicQueryPlanner;
    use crate::plan::QueryPlanner;
    use crate::record::field::Spec;
    use crate::scan::ScanControl;

    #[test]
    fn test_basic_update_planner() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let num_buffers = 100;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let planner = BasicUpdatePlanner::new(db.metadata_manager.clone());
        let updated = planner.execute_update(
            &UpdateCommand::Create(CreateCommand::Table(
                "table1".to_string(),
                vec![
                    FieldDefinition::new("A".to_string(), Spec::I32),
                    FieldDefinition::new("B".to_string(), Spec::VarChar(20)),
                ],
            )),
            tx.clone(),
        )?;
        assert_eq!(updated, 0);

        for i in 0..10 {
            let updated = planner.execute_update(
                &UpdateCommand::Insert(
                    "table1".to_string(),
                    vec!["A".to_string(), "B".to_string()],
                    vec![Value::I32(i % 2), Value::String(i.to_string())],
                ),
                tx.clone(),
            )?;
            assert_eq!(updated, 1);
        }
        let deleted = planner.execute_update(
            &UpdateCommand::Delete(
                "table1".to_string(),
                Some(Predicate::new(vec![Term::Equality(
                    Expression::Field("A".to_string()),
                    Expression::I32Constant(0),
                )])),
            ),
            tx.clone(),
        )?;
        assert_eq!(deleted, 5);

        let planner = BasicQueryPlanner::new(db.metadata_manager.clone());
        let mut query = planner.create_plan(
            &QueryData::new(vec!["B".to_string()], vec!["table1".to_string()], None),
            tx.clone(),
        )?;
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
