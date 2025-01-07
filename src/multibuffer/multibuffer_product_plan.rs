use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
};

use crate::{
    errors::TransactionError,
    materialization::{materialize_plan::MaterializePlan, temp_table::TempTable},
    plan::{Plan, PlanControl},
    record::schema::Schema,
    scan::{Scan, ScanControl},
    tx::transaction::Transaction,
};

use super::multibuffer_product_scan::MultiBufferProductScan;

pub struct MultiBufferProductPlan {
    tx: Arc<Mutex<Transaction>>,
    lhs: MaterializePlan,
    rhs: RefCell<Option<Box<Plan>>>,
    schema: Schema,
}

impl MultiBufferProductPlan {
    pub fn new(tx: Arc<Mutex<Transaction>>, lhs: Plan, rhs: Plan) -> Self {
        let lhs = MaterializePlan::new(lhs, tx.clone());
        let mut schema = Schema::new();
        schema.add_all(lhs.schema());
        schema.add_all(rhs.schema());

        Self {
            tx,
            lhs,
            rhs: RefCell::new(Some(Box::new(rhs))),
            schema,
        }
    }

    fn copy_records_from_rhs(&mut self) -> Result<TempTable, TransactionError> {
        let mut rhs_plan = self.rhs.borrow_mut().take().unwrap();
        let mut scan = rhs_plan.open(self.tx.clone())?;
        let temp_table = TempTable::new(self.tx.clone(), rhs_plan.schema());
        let mut temp_table_scan = temp_table.open()?;
        while scan.next()? {
            temp_table_scan.insert()?;
            for field_name in rhs_plan.schema().get_fields() {
                let value = scan.get_value(&field_name)?;
                temp_table_scan.set_value(&field_name, &value)?;
            }
        }
        temp_table_scan.before_first()?;
        self.rhs.replace(Some(rhs_plan));
        Ok(temp_table)
    }
}

impl PlanControl for MultiBufferProductPlan {
    fn get_num_accessed_blocks(&self) -> usize {
        let materialized_plan =
            MaterializePlan::new(*self.rhs.borrow_mut().take().unwrap(), self.tx.clone());
        let rhs_table_size = materialized_plan.get_num_accessed_blocks();
        self.rhs.replace(Some(materialized_plan.base_plan));

        let num_availables = self.tx.lock().unwrap().get_num_available_buffers();
        let lhs_blocks = self.lhs.get_num_accessed_blocks();
        let rhs_blocks = self
            .rhs
            .borrow()
            .as_ref()
            .unwrap()
            .get_num_accessed_blocks();
        let num_chunks = (rhs_table_size + num_availables - 1) / num_availables;
        rhs_blocks + lhs_blocks * num_chunks
    }

    fn get_num_output_records(&self) -> usize {
        self.lhs.get_num_output_records()
            * self.rhs.borrow().as_ref().unwrap().get_num_output_records()
    }

    fn num_distinct_values(&self, field_name: &str) -> usize {
        if self.lhs.schema().has_field(field_name) {
            self.lhs.num_distinct_values(field_name)
        } else {
            self.rhs
                .borrow()
                .as_ref()
                .unwrap()
                .num_distinct_values(field_name)
        }
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn open(&mut self, tx: Arc<Mutex<Transaction>>) -> Result<Scan, TransactionError> {
        let lhs = self.lhs.open(tx.clone())?;
        let table = self.copy_records_from_rhs()?;
        let scan =
            MultiBufferProductScan::new(tx, lhs, table.get_table_name(), table.get_layout())?;
        Ok(Scan::from(scan))
    }
}
