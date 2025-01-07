use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    index::IndexControl,
    metadata::MetadataManager,
    parser::{
        predicate::{Expression, Predicate},
        statement::{CreateCommand, UpdateCommand},
    },
    plan::{select_plan::SelectPlan, table_plan::TablePlan, Plan, PlanControl},
    planner::UpdatePlanner,
    record::{field::Value, schema::Schema},
    scan::{RecordId, Scan, ScanControl},
    tx::transaction::Transaction,
};

pub struct IndexUpdatePlanner {
    metadata_manager: Arc<Mutex<MetadataManager>>,
}

impl IndexUpdatePlanner {
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

        // First, insert a new record into the table
        let table_scan = table_plan.open(tx.clone())?;
        let mut table_scan = match table_scan {
            Scan::TableScan(table_scan) => table_scan,
            _ => panic!("Expected TableScan"),
        };

        table_scan.insert()?;
        let record_pointer = table_scan.get_record_pointer();

        let index_info_map = self
            .metadata_manager
            .lock()
            .unwrap()
            .get_index_info(table_name, tx.clone())?;

        for (field, value) in fields.iter().zip(values.iter()) {
            table_scan.set_value(field, value)?;

            if let Some(index_info) = index_info_map.get(field) {
                let mut index = index_info.open()?;
                index.insert(value, &RecordId::from(record_pointer))?;
            }
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
        let table_plan = Plan::from(table_plan);
        let mut plan = if let Some(predicate) = predicate {
            Plan::from(SelectPlan::new(table_plan, predicate.clone()))
        } else {
            table_plan
        };
        let index_info_map = self
            .metadata_manager
            .lock()
            .unwrap()
            .get_index_info(table_name, tx.clone())?;

        let mut scan = plan.open(tx.clone())?;
        let mut count = 0;
        while scan.next()? {
            // First, delete the record's ID from each index
            let record_pointer = scan.get_record_pointer();
            for (field_name, index_info) in index_info_map.iter() {
                let value = scan.get_value(field_name)?;
                let mut index = index_info.open()?;
                index.delete(&value, &RecordId::from(record_pointer))?;
            }

            // Then, delete the record from the table
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
        let table_plan = Plan::from(table_plan);
        let mut plan = if let Some(predicate) = predicate {
            Plan::from(SelectPlan::new(table_plan, predicate.clone()))
        } else {
            table_plan
        };

        let index_info_map = self
            .metadata_manager
            .lock()
            .unwrap()
            .get_index_info(table_name, tx.clone())?;
        let index = index_info_map.get(field_name).map(|info| info.open());
        let mut index = match index {
            Some(index_info) => Some(index_info?),
            _ => None,
        };

        let mut scan = plan.open(tx.clone())?;
        let mut count = 0;
        while scan.next()? {
            let new_value = expression.evaluate(&mut scan)?;
            let old_value = scan.get_value(field_name)?;

            // First, update the record in the table
            scan.set_value(field_name, &new_value)?;

            if let Some(index) = &mut index {
                let record_id = RecordId::from(scan.get_record_pointer());
                index.delete(&old_value, &record_id)?;
                index.insert(&new_value, &record_id)?;
            }
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
            CreateCommand::Index(index_name, table_name, field_name) => {
                let metadata_manager = self.metadata_manager.lock().unwrap();
                metadata_manager.create_index(index_name, table_name, field_name, tx)?;
            }
            _ => unimplemented!("Creation of indexes and views is not supported yet"),
        }
        Ok(0)
    }
}

impl UpdatePlanner for IndexUpdatePlanner {
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
