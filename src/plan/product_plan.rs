use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    record::schema::Schema,
    scan::{product_scan::ProductScan, Scan},
    tx::transaction::Transaction,
};

use super::{Plan, PlanControl};

pub struct ProductPlan {
    pub p1: Box<Plan>,
    pub p2: Box<Plan>,
    schema: Schema,
}

impl ProductPlan {
    pub fn new(p1: Plan, p2: Plan) -> Self {
        let mut schema = Schema::new();
        schema.add_all(&p1.schema());
        schema.add_all(&p2.schema());
        Self {
            p1: Box::new(p1),
            p2: Box::new(p2),
            schema,
        }
    }
}

impl PlanControl for ProductPlan {
    fn get_num_accessed_blocks(&self) -> usize {
        self.p1.get_num_accessed_blocks()
            + self.p1.get_num_output_records() * self.p2.get_num_accessed_blocks()
    }

    fn get_num_output_records(&self) -> usize {
        self.p1.get_num_output_records() * self.p2.get_num_output_records()
    }

    fn num_distinct_values(&self, field_name: &str) -> usize {
        self.p1
            .schema()
            .has_field(field_name)
            .then(|| self.p1.num_distinct_values(field_name))
            .unwrap_or_else(|| self.p2.num_distinct_values(field_name))
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn open(&mut self, tx: Arc<Mutex<Transaction>>) -> Result<Scan, TransactionError> {
        let s1 = self.p1.open(tx.clone())?;
        let s2 = self.p2.open(tx.clone())?;
        Ok(Scan::from(ProductScan::new(s1, s2)))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        db::SimpleDB, plan::table_plan::TablePlan, record::layout::Layout, scan::ScanControl,
    };

    use super::*;

    #[test]
    fn test_product_plan() -> Result<(), TransactionError> {
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

        let mut product_plan = ProductPlan::new(Plan::from(plan1), Plan::from(plan2));
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
                expected_values.push((i, i.to_string(), i + 2, j, j.to_string()))
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
