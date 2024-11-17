use std::sync::{Arc, Mutex};

use crate::{
    record::schema::Schema,
    scan::{product_scan::ProductScan, Scan},
    tx::{errors::TransactionError, transaction::Transaction},
};

use super::Plan;

pub struct ProductPlan {
    pub p1: Box<dyn Plan>,
    pub p2: Box<dyn Plan>,
    schema: Schema,
}

impl ProductPlan {
    pub fn new(p1: Box<dyn Plan>, p2: Box<dyn Plan>) -> Self {
        let mut schema = Schema::new();
        schema.add_all(&p1.schema());
        schema.add_all(&p2.schema());
        Self { p1, p2, schema }
    }
}

impl Plan for ProductPlan {
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

    fn open(&mut self, tx: Arc<Mutex<Transaction>>) -> Result<Box<dyn Scan>, TransactionError> {
        let s1 = self.p1.open(tx.clone())?;
        let s2 = self.p2.open(tx.clone())?;
        Ok(Box::new(ProductScan::new(s1, s2)))
    }
}
