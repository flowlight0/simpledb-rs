use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError, metadata::MetadataManager, parser::statement::QueryData,
    tx::transaction::Transaction,
};

use super::{product_plan::ProductPlan, table_plan::TablePlan, Plan, QueryPlanner};

pub struct BasicQueryPlanner {
    metadata_manager: Arc<Mutex<MetadataManager>>,
}

impl BasicQueryPlanner {
    pub fn new(metadata_manager: Arc<Mutex<MetadataManager>>) -> Self {
        Self { metadata_manager }
    }
}

impl QueryPlanner for BasicQueryPlanner {
    fn create_plan(
        &self,
        query: &QueryData,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<Box<dyn super::Plan>, TransactionError> {
        // Step 1
        let mut plans = vec![];
        for table_name in &query.tables {
            let table_plan = TablePlan::new(tx.clone(), table_name, self.metadata_manager.clone())?;
            plans.push(Box::new(table_plan));
        }

        // Step 2
        let mut plan: Box<dyn Plan> = plans.pop().unwrap();
        for next_plan in plans {
            let curr_plan = plan;
            let plan1 = Box::new(ProductPlan::new(curr_plan, next_plan));
            let num_blocks1 = plan1.get_num_accessed_blocks();
            let curr_plan = plan1.p1;
            let next_plan = plan1.p2;
            let plan2 = Box::new(ProductPlan::new(next_plan, curr_plan));
            let num_blocks2 = plan2.get_num_accessed_blocks();

            let next_plan = plan2.p1;
            let curr_plan = plan2.p2;
            if num_blocks1 < num_blocks2 {
                plan = Box::new(ProductPlan::new(curr_plan, next_plan));
            } else {
                plan = Box::new(ProductPlan::new(next_plan, curr_plan));
            }
        }

        // Step 3
        if let Some(predicate) = &query.predicate {
            plan = Box::new(super::select_plan::SelectPlan::new(plan, predicate.clone()));
        }

        // Step 4
        plan = Box::new(super::project_plan::ProjectPlan::new(
            plan,
            query.fields.clone(),
        ));

        Ok(plan)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::db::SimpleDB;

    use crate::parser::predicate::{Expression, Predicate, Term};

    use crate::record::schema::Schema;
    use crate::scan::table_scan::TableScan;
    use crate::scan::ScanControl;

    #[test]
    fn test_basic_query_planner() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let num_buffers = 100;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;

        let metadata_manager = db.metadata_manager.clone();
        let mut metadata_manager = metadata_manager.lock().unwrap();

        let mut schema1 = Schema::new();
        schema1.add_i32_field("A");
        schema1.add_string_field("B", 20);

        let mut schema2 = Schema::new();
        schema2.add_i32_field("C");

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        metadata_manager.create_table("table1", &schema1, tx.clone())?;
        metadata_manager.create_table("table2", &schema2, tx.clone())?;

        let layout1 = Arc::new(metadata_manager.get_layout("table1", tx.clone())?.unwrap());
        let layout2 = Arc::new(metadata_manager.get_layout("table2", tx.clone())?.unwrap());
        drop(metadata_manager);

        let mut table_scan = TableScan::new(tx.clone(), "table1", layout1.clone())?;
        table_scan.before_first()?;
        for i in 0..10 {
            table_scan.insert()?;
            table_scan.set_i32("A", i)?;
            table_scan.set_string("B", &i.to_string())?;
        }
        drop(table_scan);

        let mut table_scan = TableScan::new(tx.clone(), "table2", layout2.clone())?;
        table_scan.before_first()?;
        for i in 0..5 {
            table_scan.insert()?;
            table_scan.set_i32("C", i)?;
        }
        drop(table_scan);

        let query = QueryData {
            fields: vec!["B".to_string(), "C".to_string()],
            tables: vec!["table1".to_string(), "table2".to_string()],
            predicate: Some(Predicate::new(vec![Term::Equality(
                Expression::Field("A".to_string()),
                Expression::I32Constant(3),
            )])),
        };

        let planner = BasicQueryPlanner::new(db.metadata_manager.clone());
        let mut plan = planner.create_plan(&query, tx.clone())?;

        let mut field = plan.schema().get_fields();
        field.sort();
        assert_eq!(field, vec!["B", "C"]);

        let mut scan = plan.open(tx.clone())?;
        scan.before_first()?;
        for i in 0..5 {
            assert!(scan.next()?);
            assert_eq!(scan.get_string("B")?, "3");
            assert_eq!(scan.get_i32("C")?, i);
        }
        drop(scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }
}
