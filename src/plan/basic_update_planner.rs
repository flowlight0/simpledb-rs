use std::sync::{Arc, Mutex};

use crate::{
    metadata::MetadataManager,
    parser::{
        predicate::{Expression, Predicate},
        statement::{CreateCommand, UpdateCommand},
    },
    record::{field::Value, schema::Schema},
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
    ) -> Result<usize, anyhow::Error> {
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
    ) -> Result<usize, anyhow::Error> {
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
    ) -> Result<usize, anyhow::Error> {
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
    ) -> Result<usize, anyhow::Error> {
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
    ) -> Result<usize, anyhow::Error> {
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