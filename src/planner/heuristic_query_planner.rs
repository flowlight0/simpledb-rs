use std::sync::{Arc, Mutex};

use crate::errors::TransactionError;
use crate::materialization::{
    group_by_plan::GroupByPlan, record_comparator::RecordComparator, sort_plan::SortPlan,
};
use crate::metadata::MetadataManager;
use crate::parser::statement::QueryData;
use crate::plan::extend_plan::ExtendPlan;
use crate::plan::project_plan::ProjectPlan;
use crate::plan::{Plan, PlanControl};
use crate::tx::transaction::Transaction;

use super::table_planner::TablePlanner;
use super::QueryPlanner;

pub struct HeuristicQueryPlanner {
    metadata_manager: Arc<Mutex<MetadataManager>>,
}

impl HeuristicQueryPlanner {
    pub fn new(metadata_manager: Arc<Mutex<MetadataManager>>) -> Self {
        Self { metadata_manager }
    }

    fn choose_lowest_select_plan(
        &self,
        mut table_planners: Vec<TablePlanner>,
    ) -> Result<(Plan, Vec<TablePlanner>), TransactionError> {
        assert!(!table_planners.is_empty());
        let mut best_index = 0;
        let mut best_plan: Plan = table_planners[0].make_select_plan()?;

        for i in 1..table_planners.len() {
            let plan = table_planners[i].make_select_plan()?;
            if plan.get_num_output_records() < best_plan.get_num_output_records() {
                best_plan = plan;
                best_index = i;
            }
        }

        table_planners.remove(best_index);
        Ok((best_plan, table_planners))
    }

    fn choose_lowest_join_plan(
        &self,
        current_plan: Plan,
        mut table_planners: Vec<TablePlanner>,
    ) -> Result<(Option<Plan>, Vec<TablePlanner>), TransactionError> {
        assert!(!table_planners.is_empty());
        let mut best_index = None;
        let mut best_plan: Option<Plan> = None;

        for i in 0..table_planners.len() {
            if let Some(plan) = table_planners[i].make_join_plan(current_plan.clone())? {
                match best_plan {
                    Some(ref best_plan_ref) => {
                        if plan.get_num_output_records() < best_plan_ref.get_num_output_records() {
                            best_plan = Some(plan);
                            best_index = Some(i);
                        }
                    }
                    None => {
                        best_plan = Some(plan);
                        best_index = Some(i);
                    }
                }
            }
        }

        if let Some(best_index) = best_index {
            table_planners.remove(best_index);
        }
        Ok((best_plan, table_planners))
    }

    fn choose_lowest_product_plan(
        &self,
        current_plan: Plan,
        mut table_planners: Vec<TablePlanner>,
    ) -> Result<(Plan, Vec<TablePlanner>), TransactionError> {
        assert!(!table_planners.is_empty());
        let mut best_index = 0;
        let mut best_plan = table_planners[0].make_product_plan(current_plan.clone())?;

        for i in 1..table_planners.len() {
            let plan = table_planners[i].make_product_plan(current_plan.clone())?;
            if plan.get_num_output_records() < best_plan.get_num_output_records() {
                best_plan = plan;
                best_index = i;
            }
        }

        table_planners.remove(best_index);
        Ok((best_plan, table_planners))
    }
}

impl QueryPlanner for HeuristicQueryPlanner {
    fn create_plan(
        &self,
        query: &QueryData,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<Plan, TransactionError> {
        // Step 1, Create a TablePlanner object for each mentioned table
        let mut table_planners = vec![];
        for table_name in &query.tables {
            let table_planner = TablePlanner::new(
                table_name,
                &query.predicate,
                tx.clone(),
                self.metadata_manager.clone(),
            )?;
            table_planners.push(table_planner);
        }

        // Step 2, Choose the lowest size plan to begin the join order
        let (mut current_plan, mut table_planners) =
            self.choose_lowest_select_plan(table_planners)?;

        // Step 3, Repeatedly add a plan to the join order
        while !table_planners.is_empty() {
            let (new_plan, remaining_planners) =
                self.choose_lowest_join_plan(current_plan.clone(), table_planners)?;
            if let Some(new_plan) = new_plan {
                current_plan = new_plan;
                table_planners = remaining_planners;
            } else {
                let (new_plan, remaining_planners) =
                    self.choose_lowest_product_plan(current_plan.clone(), remaining_planners)?;
                current_plan = new_plan;
                table_planners = remaining_planners;
            }
        }

        // Step 4, Group by if specified
        if let Some(group_fields) = &query.group_by {
            current_plan = Plan::from(GroupByPlan::new(
                tx.clone(),
                current_plan,
                group_fields.clone(),
                query.aggregation_functions.clone(),
            ));
        }

        // Step 5, Extend fields using expressions
        for (expr, alias) in &query.extend_fields {
            current_plan = Plan::from(ExtendPlan::new(current_plan, expr.clone(), alias));
        }

        // Step 6, apply ordering if specified
        if let Some(order_fields) = &query.order_by {
            let comparator = Arc::new(RecordComparator::new(order_fields));
            current_plan = Plan::from(SortPlan::new(current_plan, tx.clone(), comparator));
        }

        // Step 7, Project on the field names
        if let Some(fields) = &query.fields {
            current_plan = Plan::from(ProjectPlan::new(current_plan, fields.clone()));
        }

        Ok(current_plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::SimpleDB,
        errors::ExecutionError,
        materialization::{aggregation_function::AggregationFn, sum_function::SumFn},
        parser::{
            expression::Expression,
            predicate::{Predicate, Term},
        },
        record::schema::Schema,
        scan::ScanControl,
    };

    #[test]
    fn test_heuristic_query_planner() -> Result<(), ExecutionError> {
        // This test only checks that the created plan generates expected outputs.
        // It doesn't check whether the query is optimized in the expected way.

        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 1024;
        let num_buffers = 100;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let table1 = "table1";
        let mut schema1 = Schema::new();
        schema1.add_i32_field("A");
        schema1.add_string_field("B", 20);

        let table2 = "table2";
        let mut schema2 = Schema::new();
        schema2.add_i32_field("C");

        {
            let mut metadata_manager = db.metadata_manager.lock().unwrap();
            metadata_manager.create_table(table1, &schema1, tx.clone())?;
            metadata_manager.create_table(table2, &schema2, tx.clone())?;
            metadata_manager.create_index("IA", table1, "A", tx.clone())?;
            metadata_manager.create_index("IC", table2, "C", tx.clone())?;
            drop(metadata_manager);
        }

        for i in 0..10 {
            let update_command = format!("INSERT INTO {} (A, B) VALUES ({}, '{}')", table1, i, i);
            db.planner
                .lock()
                .unwrap()
                .execute_update(&update_command, tx.clone())?;
        }
        for i in 5..15 {
            let update_command = format!("INSERT INTO {} (C) VALUES ({})", table2, i);
            db.planner
                .lock()
                .unwrap()
                .execute_update(&update_command, tx.clone())?;
        }

        let query = QueryData::new(
            vec!["B".to_string(), "C".to_string()],
            vec!["table1".to_string(), "table2".to_string()],
            Some(Predicate::new(vec![Term::Equality(
                Expression::Field("A".to_string()),
                Expression::Field("C".to_string()),
            )])),
        );

        let heuristic_planner = HeuristicQueryPlanner::new(db.metadata_manager.clone());
        let mut heuristic_plan = heuristic_planner.create_plan(&query, tx.clone())?;

        let mut field = heuristic_plan.schema().get_fields();
        field.sort();
        assert_eq!(field, vec!["B", "C"]);

        {
            let mut scan = heuristic_plan.open(tx.clone())?;
            scan.before_first()?;
            for i in 5..10 {
                assert!(scan.next()?);
                assert_eq!(scan.get_string("B")?, Some(i.to_string()));
                assert_eq!(scan.get_i32("C")?, Some(i));
            }
        }

        tx.lock().unwrap().commit()?;
        Ok(())
    }

    #[test]
    fn test_extend_heuristic_query_planner() -> Result<(), ExecutionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 1024;
        let num_buffers = 100;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let table = "table1";
        {
            let mut md = db.metadata_manager.lock().unwrap();
            let mut schema = Schema::new();
            schema.add_i32_field("A");
            md.create_table(table, &schema, tx.clone())?;
            md.create_index("IA", table, "A", tx.clone())?;
        }

        for i in 0..5 {
            let update_command = format!("INSERT INTO {} (A) VALUES ({})", table, i);
            db.planner
                .lock()
                .unwrap()
                .execute_update(&update_command, tx.clone())?;
        }

        let query = QueryData::new_with_order_and_extend(
            vec!["B".to_string()],
            vec![table.to_string()],
            None,
            None,
            vec![(
                Expression::Add(
                    Box::new(Expression::Field("A".to_string())),
                    Box::new(Expression::I32Constant(1)),
                ),
                "B".to_string(),
            )],
        );

        let planner = HeuristicQueryPlanner::new(db.metadata_manager.clone());
        let mut plan = planner.create_plan(&query, tx.clone())?;

        let mut scan = plan.open(tx.clone())?;
        scan.before_first()?;
        for i in 0..5 {
            assert!(scan.next()?);
            assert_eq!(scan.get_i32("B")?, Some(i + 1));
        }
        assert!(!scan.next()?);
        drop(scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }

    #[test]
    fn test_group_by_heuristic_query_planner() -> Result<(), ExecutionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 1024;
        let num_buffers = 100;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let table = "table1";
        {
            let mut md = db.metadata_manager.lock().unwrap();
            let mut schema = Schema::new();
            schema.add_i32_field("A");
            schema.add_i32_field("B");
            md.create_table(table, &schema, tx.clone())?;
            md.create_index("IA", table, "A", tx.clone())?;
        }

        for i in 0..50 {
            let update_command = format!("INSERT INTO {} (A, B) VALUES ({}, {})", table, i / 5, i);
            db.planner
                .lock()
                .unwrap()
                .execute_update(&update_command, tx.clone())?;
        }

        let query = QueryData::new_full(
            Some(vec!["A".to_string(), "B".to_string()]),
            vec![table.to_string()],
            None,
            Some(vec!["A".to_string()]),
            None,
            Vec::new(),
            vec![AggregationFn::from(SumFn::new("B"))],
        );

        let planner = HeuristicQueryPlanner::new(db.metadata_manager.clone());
        let mut plan = planner.create_plan(&query, tx.clone())?;

        let mut scan = plan.open(tx.clone())?;
        scan.before_first()?;
        for i in 0..10 {
            assert!(scan.next()?);
            assert_eq!(scan.get_i32("A")?, Some(i));
            assert_eq!(scan.get_i32("B")?, Some(25 * i + 10));
        }
        assert!(!scan.next()?);
        drop(scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }

    #[test]
    fn test_group_by_alias_heuristic_query_planner() -> Result<(), ExecutionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 1024;
        let num_buffers = 100;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let table = "table1";
        {
            let mut md = db.metadata_manager.lock().unwrap();
            let mut schema = Schema::new();
            schema.add_i32_field("A");
            schema.add_i32_field("B");
            md.create_table(table, &schema, tx.clone())?;
            md.create_index("IA", table, "A", tx.clone())?;
        }

        for i in 0..50 {
            let update_command = format!("INSERT INTO {} (A, B) VALUES ({}, {})", table, i / 5, i);
            db.planner
                .lock()
                .unwrap()
                .execute_update(&update_command, tx.clone())?;
        }

        let query = QueryData::new_full(
            Some(vec!["A".to_string(), "total".to_string()]),
            vec![table.to_string()],
            None,
            Some(vec!["A".to_string()]),
            None,
            Vec::new(),
            vec![AggregationFn::from(SumFn::new("B").with_alias("total"))],
        );

        let planner = HeuristicQueryPlanner::new(db.metadata_manager.clone());
        let mut plan = planner.create_plan(&query, tx.clone())?;

        let mut scan = plan.open(tx.clone())?;
        scan.before_first()?;
        for i in 0..10 {
            assert!(scan.next()?);
            assert_eq!(scan.get_i32("A")?, Some(i));
            assert_eq!(scan.get_i32("total")?, Some(25 * i + 10));
        }
        assert!(!scan.next()?);
        drop(scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }
}
