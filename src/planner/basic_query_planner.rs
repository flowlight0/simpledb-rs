use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    materialization::{
        group_by_plan::GroupByPlan, record_comparator::RecordComparator, sort_plan::SortPlan,
    },
    metadata::MetadataManager,
    parser::statement::QueryData,
    plan::{
        extend_plan::ExtendPlan, product_plan::ProductPlan, project_plan::ProjectPlan,
        select_plan::SelectPlan, table_plan::TablePlan, Plan, PlanControl,
    },
    tx::transaction::Transaction,
};

use super::QueryPlanner;

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
    ) -> Result<Plan, TransactionError> {
        // Step 1
        let mut plans = vec![];
        for table_name in &query.tables {
            let table_plan = TablePlan::new(tx.clone(), table_name, self.metadata_manager.clone())?;
            plans.push(Plan::from(table_plan));
        }

        // Step 2
        let mut plan: Plan = plans.pop().unwrap();
        for next_plan in plans {
            let curr_plan = plan;
            let plan1 = ProductPlan::new(curr_plan, next_plan);
            let num_blocks1 = plan1.get_num_accessed_blocks();
            let curr_plan = plan1.p1;
            let next_plan = plan1.p2;
            let plan2 = Box::new(ProductPlan::new(*next_plan, *curr_plan));
            let num_blocks2 = plan2.get_num_accessed_blocks();

            let next_plan = plan2.p1;
            let curr_plan = plan2.p2;
            if num_blocks1 < num_blocks2 {
                plan = Plan::from(ProductPlan::new(*curr_plan, *next_plan));
            } else {
                plan = Plan::from(ProductPlan::new(*next_plan, *curr_plan));
            }
        }

        // Step 3
        if let Some(predicate) = &query.predicate {
            plan = Plan::from(SelectPlan::new(plan, predicate.clone()));
        }

        // Step 4
        if let Some(group_fields) = &query.group_by {
            plan = Plan::from(GroupByPlan::new(
                tx.clone(),
                plan,
                group_fields.clone(),
                query.aggregation_functions.clone(),
            ));
        }

        // Step 5
        for (expr, alias) in &query.extend_fields {
            plan = Plan::from(ExtendPlan::new(plan, expr.clone(), alias));
        }

        // Step 6
        if let Some(order_fields) = &query.order_by {
            let comparator = Arc::new(RecordComparator::new(order_fields));
            plan = Plan::from(SortPlan::new(plan, tx.clone(), comparator));
        }

        // Step 7
        if let Some(fields) = &query.fields {
            plan = Plan::from(ProjectPlan::new(plan, fields.clone()));
        }

        Ok(plan)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::db::SimpleDB;

    use crate::parser::{
        expression::Expression,
        predicate::{Predicate, Term},
    };

    use crate::materialization::{aggregation_function::AggregationFn, sum_function::SumFn};
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

        let query = QueryData::new(
            vec!["B".to_string(), "C".to_string()],
            vec!["table1".to_string(), "table2".to_string()],
            Some(Predicate::new(vec![Term::Equality(
                Expression::Field("A".to_string()),
                Expression::I32Constant(3),
            )])),
        );

        let planner = BasicQueryPlanner::new(db.metadata_manager.clone());
        let mut plan = planner.create_plan(&query, tx.clone())?;

        let mut field = plan.schema().get_fields();
        field.sort();
        assert_eq!(field, vec!["B", "C"]);

        let mut scan = plan.open(tx.clone())?;
        scan.before_first()?;
        for i in 0..5 {
            assert!(scan.next()?);
            assert_eq!(scan.get_string("B")?, Some("3".to_string()));
            assert_eq!(scan.get_i32("C")?, Some(i));
        }
        drop(scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }

    #[test]
    fn test_select_all_basic_query_planner() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let num_buffers = 100;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        {
            let mut metadata_manager = db.metadata_manager.lock().unwrap();
            let mut schema = Schema::new();
            schema.add_i32_field("A");
            schema.add_string_field("B", 20);
            metadata_manager.create_table("table1", &schema, tx.clone())?;
        }

        {
            let layout = {
                let md = db.metadata_manager.lock().unwrap();
                md.get_layout("table1", tx.clone())?.unwrap()
            };
            let mut table_scan = TableScan::new(tx.clone(), "table1", Arc::new(layout))?;
            table_scan.before_first()?;
            for i in 0..5 {
                table_scan.insert()?;
                table_scan.set_i32("A", i)?;
                table_scan.set_string("B", &i.to_string())?;
            }
            drop(table_scan);
        }

        let query = QueryData::new_all(vec!["table1".to_string()], None);

        let planner = BasicQueryPlanner::new(db.metadata_manager.clone());
        let mut plan = planner.create_plan(&query, tx.clone())?;

        let mut fields = plan.schema().get_fields();
        fields.sort();
        assert_eq!(fields, vec!["A", "B"]);

        let mut scan = plan.open(tx.clone())?;
        let mut count = 0;
        scan.before_first()?;
        while scan.next()? {
            assert!(scan.has_field("A"));
            assert!(scan.has_field("B"));
            count += 1;
        }
        assert_eq!(count, 5);
        drop(scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }

    #[test]
    fn test_order_by_basic_query_planner() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let num_buffers = 100;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        {
            let mut metadata_manager = db.metadata_manager.lock().unwrap();
            let mut schema = Schema::new();
            schema.add_i32_field("A");
            metadata_manager.create_table("table1", &schema, tx.clone())?;
        }

        {
            let layout = {
                let md = db.metadata_manager.lock().unwrap();
                md.get_layout("table1", tx.clone())?.unwrap()
            };
            let mut table_scan = TableScan::new(tx.clone(), "table1", Arc::new(layout))?;
            table_scan.before_first()?;
            for i in (0..5).rev() {
                table_scan.insert()?;
                table_scan.set_i32("A", i)?;
            }
            drop(table_scan);
        }

        let query = QueryData::new_all_with_order(
            vec!["table1".to_string()],
            None,
            Some(vec!["A".to_string()]),
        );

        let planner = BasicQueryPlanner::new(db.metadata_manager.clone());
        let mut plan = planner.create_plan(&query, tx.clone())?;

        let mut scan = plan.open(tx.clone())?;
        scan.before_first()?;
        for i in 0..5 {
            assert!(scan.next()?);
            assert_eq!(scan.get_i32("A")?, Some(i));
        }
        assert!(!scan.next()?);
        drop(scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }

    #[test]
    fn test_extend_basic_query_planner() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let num_buffers = 100;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        {
            let mut md = db.metadata_manager.lock().unwrap();
            let mut schema = Schema::new();
            schema.add_i32_field("A");
            md.create_table("table1", &schema, tx.clone())?;
        }

        {
            let layout = {
                let md = db.metadata_manager.lock().unwrap();
                md.get_layout("table1", tx.clone())?.unwrap()
            };
            let mut table_scan = TableScan::new(tx.clone(), "table1", Arc::new(layout))?;
            table_scan.before_first()?;
            for i in 0..5 {
                table_scan.insert()?;
                table_scan.set_i32("A", i)?;
            }
        }

        let query = QueryData::new_with_order_and_extend(
            vec!["A".to_string(), "B".to_string()],
            vec!["table1".to_string()],
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

        let planner = BasicQueryPlanner::new(db.metadata_manager.clone());
        let mut plan = planner.create_plan(&query, tx.clone())?;

        let mut scan = plan.open(tx.clone())?;
        scan.before_first()?;
        for i in 0..5 {
            assert!(scan.next()?);
            assert_eq!(scan.get_i32("A")?, Some(i));
            assert_eq!(scan.get_i32("B")?, Some(i + 1));
        }
        assert!(!scan.next()?);
        drop(scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }

    #[test]
    fn test_group_by_basic_query_planner() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let num_buffers = 100;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        {
            let mut md = db.metadata_manager.lock().unwrap();
            let mut schema = Schema::new();
            schema.add_i32_field("A");
            schema.add_i32_field("B");
            md.create_table("table1", &schema, tx.clone())?;
        }

        {
            let layout = {
                let md = db.metadata_manager.lock().unwrap();
                md.get_layout("table1", tx.clone())?.unwrap()
            };
            let mut table_scan = TableScan::new(tx.clone(), "table1", Arc::new(layout))?;
            table_scan.before_first()?;
            for i in 0..50 {
                table_scan.insert()?;
                table_scan.set_i32("A", i / 5)?;
                table_scan.set_i32("B", i)?;
            }
        }

        let query = QueryData::new_full(
            Some(vec!["A".to_string(), "B".to_string()]),
            vec!["table1".to_string()],
            None,
            Some(vec!["A".to_string()]),
            None,
            Vec::new(),
            vec![AggregationFn::from(SumFn::new("B"))],
        );

        let planner = BasicQueryPlanner::new(db.metadata_manager.clone());
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
    fn test_group_by_alias_basic_query_planner() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let num_buffers = 100;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        {
            let mut md = db.metadata_manager.lock().unwrap();
            let mut schema = Schema::new();
            schema.add_i32_field("A");
            schema.add_i32_field("B");
            md.create_table("table1", &schema, tx.clone())?;
        }

        {
            let layout = {
                let md = db.metadata_manager.lock().unwrap();
                md.get_layout("table1", tx.clone())?.unwrap()
            };
            let mut table_scan = TableScan::new(tx.clone(), "table1", Arc::new(layout))?;
            table_scan.before_first()?;
            for i in 0..50 {
                table_scan.insert()?;
                table_scan.set_i32("A", i / 5)?;
                table_scan.set_i32("B", i)?;
            }
        }

        let query = QueryData::new_full(
            Some(vec!["A".to_string(), "total".to_string()]),
            vec!["table1".to_string()],
            None,
            Some(vec!["A".to_string()]),
            None,
            vec![(Expression::Field("B".to_string()), "total".to_string())],
            vec![AggregationFn::from(SumFn::new("B"))],
        );

        let planner = BasicQueryPlanner::new(db.metadata_manager.clone());
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
