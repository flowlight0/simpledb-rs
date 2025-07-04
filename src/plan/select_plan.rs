use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    parser::predicate::Predicate,
    record::schema::Schema,
    scan::{select_scan::SelectScan, Scan},
    tx::transaction::Transaction,
};

use super::{Plan, PlanControl};

#[derive(Clone)]
pub struct SelectPlan {
    plan: Box<Plan>,
    predicate: Predicate,
}

impl SelectPlan {
    pub fn new(plan: Plan, predicate: Predicate) -> Self {
        SelectPlan {
            plan: Box::new(plan),
            predicate,
        }
    }
}

impl PlanControl for SelectPlan {
    fn get_num_accessed_blocks(&self) -> usize {
        self.plan.get_num_accessed_blocks()
    }

    fn get_num_output_records(&self) -> usize {
        self.plan.get_num_output_records() / self.predicate.get_reduction_factor(&self.plan)
    }

    fn num_distinct_values(&self, field_name: &str) -> usize {
        if self.predicate.equates_with_constant(field_name).is_some() {
            1
        } else {
            let another_field = self.predicate.equates_with_field(field_name);
            if another_field.is_some() {
                let l_distinct = self.plan.num_distinct_values(field_name);
                let r_distinct = self.plan.num_distinct_values(&another_field.unwrap());
                std::cmp::min(l_distinct, r_distinct)
            } else {
                self.plan.num_distinct_values(field_name)
            }
        }
    }

    fn schema(&self) -> &Schema {
        self.plan.schema()
    }

    fn open(&mut self, tx: Arc<Mutex<Transaction>>) -> Result<Scan, TransactionError> {
        let base_scan = self.plan.open(tx)?;
        Ok(Scan::from(SelectScan::new(
            base_scan,
            self.predicate.clone(),
        )))
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::db::SimpleDB;

    use crate::parser::{expression::Expression, predicate::Term};
    use crate::plan::select_plan;
    use crate::plan::table_plan::TablePlan;
    use crate::record::layout::Layout;
    use crate::record::schema::Schema;
    use crate::scan::table_scan::TableScan;
    use crate::scan::ScanControl;

    #[test]
    fn test_select_plan() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 1024;
        let num_buffers = 3;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);
        let layout = Arc::new(Layout::new(schema));

        let table_name = "testtable";
        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let mut metadata_manager = db.metadata_manager.lock().unwrap();
        metadata_manager.create_table(table_name, &layout.schema, tx.clone())?;
        {
            let mut table_scan = TableScan::new(tx.clone(), table_name, layout.clone())?;
            table_scan.before_first()?;
            for i in 0..200 {
                table_scan.insert()?;
                table_scan.set_i32("A", i)?;
                table_scan.set_string("B", &i.to_string())?;
            }
        }
        tx.lock().unwrap().commit()?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        metadata_manager
            .stat_manager
            .lock()
            .unwrap()
            .refresh_statistics(tx.clone())?;
        drop(metadata_manager);

        let table_plan = TablePlan::new(tx.clone(), table_name, db.metadata_manager)?;
        let select_plan = select_plan::SelectPlan::new(
            Plan::from(table_plan),
            Predicate::new(vec![Term::Equality(
                Expression::Field("A".to_string()),
                Expression::I32Constant(1),
            )]),
        );

        assert!(select_plan.get_num_accessed_blocks() > 0);
        assert!(select_plan.get_num_output_records() < 200);
        assert!(select_plan.num_distinct_values("A") > 0);

        Ok(())
    }
}
