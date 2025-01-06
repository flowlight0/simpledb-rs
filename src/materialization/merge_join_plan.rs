use std::{
    cmp::max,
    sync::{Arc, Mutex},
};

use crate::{
    errors::TransactionError,
    plan::{Plan, PlanControl},
    record::schema::Schema,
    scan::Scan,
    tx::transaction::Transaction,
};

use super::{
    merge_join_scan::MergeJoinScan, record_comparator::RecordComparator, sort_plan::SortPlan,
};

pub struct MergeJoinPlan {
    p1: SortPlan,
    p2: SortPlan,
    schema: Schema,
    field_name1: String,
    field_name2: String,
}

impl MergeJoinPlan {
    pub fn new(
        tx: Arc<Mutex<Transaction>>,
        p1: Plan,
        p2: Plan,
        field_name1: &str,
        field_name2: &str,
    ) -> Self {
        let comparator = Arc::new(RecordComparator::new(&vec![field_name1.to_string()]));
        let p1 = SortPlan::new(p1, tx.clone(), comparator);

        let comparator = Arc::new(RecordComparator::new(&vec![field_name2.to_string()]));
        let p2 = SortPlan::new(p2, tx.clone(), comparator);

        let mut schema = Schema::new();
        schema.add_all(&p1.schema());
        schema.add_all(&p2.schema());
        MergeJoinPlan {
            p1,
            p2,
            schema,
            field_name1: field_name1.to_string(),
            field_name2: field_name2.to_string(),
        }
    }
}

impl PlanControl for MergeJoinPlan {
    fn get_num_accessed_blocks(&self) -> usize {
        self.p1.get_num_accessed_blocks() + self.p2.get_num_accessed_blocks()
    }

    fn get_num_output_records(&self) -> usize {
        let p1_distinct_values = self.p1.num_distinct_values(&self.field_name1);
        let p2_distinct_values = self.p2.num_distinct_values(&self.field_name2);
        self.p1.get_num_output_records() * self.p2.get_num_output_records()
            / max(p1_distinct_values, p2_distinct_values)
    }

    fn num_distinct_values(&self, field_name: &str) -> usize {
        if self.p1.schema().has_field(field_name) {
            self.p1.num_distinct_values(field_name)
        } else {
            self.p2.num_distinct_values(field_name)
        }
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn open(&mut self, tx: Arc<Mutex<Transaction>>) -> Result<Scan, TransactionError> {
        let s1 = self.p1.open(tx.clone())?;
        let s2 = self.p2.open(tx.clone())?;
        match (s1, s2) {
            (Scan::SortScan(s1), Scan::SortScan(s2)) => Ok(Scan::from(MergeJoinScan::new(
                s1,
                s2,
                &self.field_name1,
                &self.field_name2,
            )?)),
            _ => panic!("Expected SortScan"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{
        db::SimpleDB,
        errors::TransactionError,
        plan::{table_plan::TablePlan, Plan, PlanControl},
        record::schema::Schema,
        scan::ScanControl,
    };

    use super::MergeJoinPlan;

    #[test]
    fn test_merge_join_plan() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 1024;
        let num_buffers = 100;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let mut schema1 = Schema::new();
        schema1.add_i32_field("A");
        schema1.add_string_field("B", 20);

        let mut schema2 = Schema::new();
        schema2.add_i32_field("C");
        schema2.add_string_field("D", 20);

        db.metadata_manager
            .lock()
            .unwrap()
            .create_table("test_table1", &schema1, tx.clone())?;
        let mut p1 = TablePlan::new(tx.clone(), "test_table1", db.metadata_manager.clone())?;
        {
            let mut table_scan = p1.open(tx.clone())?;
            for i in 0..10 {
                table_scan.insert()?;
                table_scan.set_i32("A", i as i32)?;
                table_scan.set_string("B", &(i / 2).to_string())?;
            }
        }

        db.metadata_manager
            .lock()
            .unwrap()
            .create_table("test_table2", &schema2, tx.clone())?;
        let mut p2 = TablePlan::new(tx.clone(), "test_table2", db.metadata_manager.clone())?;
        {
            let mut table_scan = p2.open(tx.clone())?;
            for i in 0..10 {
                table_scan.insert()?;
                table_scan.set_i32("C", i as i32)?;
                table_scan.set_string("D", &(i / 2).to_string())?;
            }
        }

        let mut merge_join_plan =
            MergeJoinPlan::new(tx.clone(), Plan::from(p1), Plan::from(p2), "B", "D");
        let mut merge_join_scan = merge_join_plan.open(tx.clone())?;

        let mut expected_values = vec![];
        for i in 0..5 {
            expected_values.push((i * 2 + 0, i.to_string(), i * 2 + 0, i.to_string()));
            expected_values.push((i * 2 + 0, i.to_string(), i * 2 + 1, i.to_string()));
            expected_values.push((i * 2 + 1, i.to_string(), i * 2 + 0, i.to_string()));
            expected_values.push((i * 2 + 1, i.to_string(), i * 2 + 1, i.to_string()));
        }
        expected_values.sort();

        let mut actual_values = vec![];
        while merge_join_scan.next()? {
            let a = merge_join_scan.get_i32("A")?;
            let b = merge_join_scan.get_string("B")?;
            let c = merge_join_scan.get_i32("C")?;
            let d = merge_join_scan.get_string("D")?;
            actual_values.push((a, b, c, d));
        }
        actual_values.sort();

        assert_eq!(expected_values, actual_values);
        Ok(())
    }
}
