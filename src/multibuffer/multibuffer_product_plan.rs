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

#[cfg(test)]
mod tests {
    use crate::{
        db::SimpleDB, plan::table_plan::TablePlan, record::layout::Layout, scan::ScanControl,
    };

    use super::*;

    #[test]
    fn test_multibuffer_product_plan() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let num_buffers = 10;

        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let mut schema1 = Schema::new();
        schema1.add_i32_field("A");
        schema1.add_string_field("B", 20);
        schema1.add_i32_field("C");
        let layout1 = Arc::new(Layout::new(schema1));

        let mut schema2 = Schema::new();
        schema2.add_i32_field("D");
        schema2.add_string_field("E", 20);
        let layout2 = Arc::new(Layout::new(schema2));

        let table1 = "test_table1";
        let table1_size = 30;
        db.metadata_manager
            .lock()
            .unwrap()
            .create_table(table1, &layout1.schema, tx.clone())?;

        let mut plan1 = TablePlan::new(tx.clone(), table1, db.metadata_manager.clone())?;
        {
            let mut scan1 = plan1.open(tx.clone())?;
            scan1.before_first()?;
            for i in 0..table1_size {
                scan1.insert()?;
                scan1.set_i32("A", i)?;
                scan1.set_string("B", &i.to_string())?;
                scan1.set_i32("C", i + 2)?;
            }
        }

        let table2 = "test_table2";
        let table2_size = 30;
        db.metadata_manager
            .lock()
            .unwrap()
            .create_table(table2, &layout2.schema, tx.clone())?;
        let mut plan2 = TablePlan::new(tx.clone(), table2, db.metadata_manager.clone())?;
        {
            let mut scan2 = plan2.open(tx.clone())?;
            scan2.before_first()?;
            for i in 0..table2_size {
                scan2.insert()?;
                scan2.set_i32("D", i)?;
                scan2.set_string("E", &i.to_string())?;
            }
        }

        let mut product_plan =
            MultiBufferProductPlan::new(tx.clone(), Plan::from(plan1), Plan::from(plan2));
        let mut product_scan = product_plan.open(tx.clone())?;
        product_scan.before_first()?;

        let mut actual_values = vec![];
        let mut expected_values = vec![];
        for i in 0..table1_size {
            for j in 0..table2_size {
                assert!(product_scan.next()?);
                actual_values.push((
                    product_scan.get_i32("A")?,
                    product_scan.get_string("B")?,
                    product_scan.get_i32("C")?,
                    product_scan.get_i32("D")?,
                    product_scan.get_string("E")?,
                ));
                expected_values.push((i, i.to_string(), i + 2, j, j.to_string()));
            }
        }
        expected_values.sort();
        actual_values.sort();
        assert_eq!(expected_values, actual_values);

        assert!(!product_scan.next()?);
        drop(product_scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }
}
