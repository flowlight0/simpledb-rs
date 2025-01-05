use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    plan::{Plan, PlanControl},
    record::schema::Schema,
    scan::Scan,
    tx::transaction::Transaction,
};

use super::{
    aggregation_function::{AggregationFn, AggregationFnControl},
    group_by_scan::GroupByScan,
    record_comparator::RecordComparator,
    sort_plan::SortPlan,
};

pub struct GroupByPlan {
    sort_plan: Box<Plan>,
    group_fields: Vec<String>,
    aggregation_functions: Vec<AggregationFn>,
    schema: Schema,
}

impl GroupByPlan {
    pub fn new(
        tx: Arc<Mutex<Transaction>>,
        plan: Plan,
        group_fields: Vec<String>,
        aggregation_functions: Vec<AggregationFn>,
    ) -> Self {
        let mut schema = Schema::new();
        for field_name in &group_fields {
            schema.add_field(field_name, &plan.schema().get_field_spec(field_name));
        }
        for aggregation_fn in &aggregation_functions {
            let field_name = aggregation_fn.get_field_name();
            schema.add_field(&field_name, &plan.schema().get_field_spec(field_name));
        }

        let comparator = Arc::new(RecordComparator::new(&group_fields));
        let sort_plan = Plan::from(SortPlan::new(plan, tx, comparator));

        Self {
            sort_plan: Box::new(sort_plan),
            group_fields,
            aggregation_functions,
            schema,
        }
    }
}

impl PlanControl for GroupByPlan {
    fn get_num_accessed_blocks(&self) -> usize {
        self.sort_plan.get_num_accessed_blocks()
    }

    fn get_num_output_records(&self) -> usize {
        let mut num_groups = 1;
        for field_name in &self.group_fields {
            num_groups *= self.num_distinct_values(field_name);
        }
        num_groups
    }

    fn num_distinct_values(&self, field_name: &str) -> usize {
        if self.sort_plan.schema().has_field(field_name) {
            self.sort_plan.num_distinct_values(field_name)
        } else {
            self.get_num_output_records()
        }
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn open(&mut self, tx: Arc<Mutex<Transaction>>) -> Result<Scan, TransactionError> {
        let s = self.sort_plan.open(tx.clone())?;
        match s {
            Scan::SortScan(s) => Ok(Scan::from(GroupByScan::new(
                s,
                &self.group_fields,
                &self.aggregation_functions,
            )?)),
            _ => panic!("SortPlan did not return a SortScan"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{
        db::SimpleDB,
        errors::TransactionError,
        materialization::{aggregation_function::AggregationFn, max_function::MaxFn},
        plan::{table_plan::TablePlan, Plan, PlanControl},
        record::schema::Schema,
        scan::ScanControl,
    };

    use super::GroupByPlan;

    #[test]
    fn test_group_by_plan() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);

        let table_name = "test_table";
        db.metadata_manager
            .lock()
            .unwrap()
            .create_table("test_table", &schema, tx.clone())?;

        let mut base_plan = TablePlan::new(tx.clone(), table_name, db.metadata_manager.clone())?;

        {
            let mut base_scan = base_plan.open(tx.clone())?;
            base_scan.before_first()?;
            for i in 0..50 {
                base_scan.insert()?;
                base_scan.set_i32("A", -i)?;
                base_scan.set_string("B", &format!("B{}", i % 5))?;
            }
        }

        let group_fields = vec!["B".to_string()];
        let aggregation_functions = vec![AggregationFn::from(MaxFn::new("A"))];
        let mut group_by_plan = GroupByPlan::new(
            tx.clone(),
            Plan::from(base_plan),
            group_fields,
            aggregation_functions,
        );
        let mut scan = group_by_plan.open(tx.clone())?;
        scan.before_first()?;
        for i in 0..5 {
            assert!(scan.next()?);
            assert_eq!(scan.get_i32("A")?, -i);
            assert_eq!(scan.get_string("B")?, format!("B{}", i % 5));
        }
        assert!(!scan.next()?);

        Ok(())
    }
}
