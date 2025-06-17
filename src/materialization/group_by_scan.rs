use std::collections::HashMap;

use crate::{
    errors::TransactionError,
    record::field::Value,
    scan::{Scan, ScanControl},
};

use super::{
    aggregation_function::{AggregationFn, AggregationFnControl},
    sort_scan::SortScan,
};

pub struct GroupByScan {
    scan: Box<Scan>,
    group_fields: Vec<String>,
    aggregation_functions: Vec<AggregationFn>,
    more_group: bool,
    group_values: HashMap<String, Value>,
}

impl GroupByScan {
    pub fn new(
        sort_scan: SortScan,
        group_fields: &Vec<String>,
        aggregation_functions: &Vec<AggregationFn>,
    ) -> Result<Self, TransactionError> {
        let mut group_by_scan = GroupByScan {
            scan: Box::new(Scan::from(sort_scan)),
            group_fields: group_fields.clone(),
            aggregation_functions: aggregation_functions.clone(),
            more_group: false,
            group_values: HashMap::new(),
        };
        group_by_scan.before_first()?;
        Ok(group_by_scan)
    }
}

impl ScanControl for GroupByScan {
    fn before_first(&mut self) -> Result<(), TransactionError> {
        self.scan.before_first()?;
        self.more_group = self.scan.next()?;
        Ok(())
    }

    fn after_last(&mut self) -> Result<(), TransactionError> {
        self.scan.after_last()?;
        self.more_group = self.scan.previous()?;
        Ok(())
    }

    fn next(&mut self) -> Result<bool, TransactionError> {
        if !self.more_group {
            return Ok(false);
        }

        for aggregation_fn in &mut self.aggregation_functions {
            aggregation_fn.process_first(&mut self.scan)?;
        }

        self.group_values = HashMap::new();
        for field_name in &self.group_fields {
            let value = self.scan.get_value(field_name)?;
            self.group_values.insert(field_name.to_string(), value);
        }

        while self.scan.next()? {
            self.more_group = true;

            let mut new_group_values = HashMap::new();
            for field_name in &self.group_fields {
                let value = self.scan.get_value(field_name)?;
                new_group_values.insert(field_name.to_string(), value);
            }

            if new_group_values != self.group_values {
                return Ok(true);
            }
            for aggregation_fn in &mut self.aggregation_functions {
                aggregation_fn.process_next(&mut self.scan)?;
            }
        }
        self.more_group = false;
        Ok(true)
    }

    fn previous(&mut self) -> Result<bool, TransactionError> {
        if !self.more_group {
            return Ok(false);
        }

        for aggregation_fn in &mut self.aggregation_functions {
            aggregation_fn.process_first(&mut self.scan)?;
        }

        self.group_values = HashMap::new();
        for field_name in &self.group_fields {
            let value = self.scan.get_value(field_name)?;
            self.group_values.insert(field_name.to_string(), value);
        }

        while self.scan.previous()? {
            self.more_group = true;

            let mut new_group_values = HashMap::new();
            for field_name in &self.group_fields {
                let value = self.scan.get_value(field_name)?;
                new_group_values.insert(field_name.to_string(), value);
            }

            if new_group_values != self.group_values {
                return Ok(true);
            }

            for aggregation_fn in &mut self.aggregation_functions {
                aggregation_fn.process_next(&mut self.scan)?;
            }
        }

        self.more_group = false;
        Ok(true)
    }

    fn get_i32(&mut self, field_name: &str) -> Result<i32, TransactionError> {
        match self.get_value(field_name)? {
            Value::I32(i) => Ok(i),
            _ => panic!("Field {} is not an integer", field_name),
        }
    }

    fn get_string(&mut self, field_name: &str) -> Result<String, TransactionError> {
        match self.get_value(field_name)? {
            Value::String(s) => Ok(s),
            _ => panic!("Field {} is not a string", field_name),
        }
    }

    fn get_value(&mut self, field_name: &str) -> Result<Value, TransactionError> {
        if self.group_fields.contains(&field_name.to_string()) {
            return Ok(self.group_values.get(field_name).unwrap().clone());
        } else {
            for aggregation_fn in &self.aggregation_functions {
                if aggregation_fn.get_field_name() == field_name {
                    return Ok(aggregation_fn.get_value().unwrap());
                }
            }
            panic!("Field {} not found", field_name);
        }
    }

    fn has_field(&self, field_name: &str) -> bool {
        if self.group_fields.contains(&field_name.to_string()) {
            return true;
        }

        for aggregation_fn in &self.aggregation_functions {
            if aggregation_fn.get_field_name() == field_name {
                return true;
            }
        }
        return false;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{
        db::SimpleDB,
        materialization::{
            max_function::MaxFn, record_comparator::RecordComparator, temp_table::TempTable,
        },
        record::schema::Schema,
    };

    use super::*;

    #[test]
    fn test_group_by_scan() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 1024;
        let num_buffers = 100;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_i32_field("B");

        let temp_table = TempTable::new(tx.clone(), &schema);

        {
            let mut scan = temp_table.open()?;
            scan.before_first()?;
            for i in 0..50 {
                scan.insert()?;
                scan.set_i32("A", i / 5)?;
                scan.set_i32("B", i)?;
            }
        }

        let group_fields = vec!["A".to_string()];
        let sort_scan = SortScan::new(
            vec![temp_table],
            Arc::new(RecordComparator::new(&group_fields)),
        )?;
        let aggregation_functions = vec![AggregationFn::from(MaxFn::new("B"))];
        let mut group_by_scan = GroupByScan::new(sort_scan, &group_fields, &aggregation_functions)?;

        group_by_scan.before_first()?;
        for i in 0..10 {
            assert!(group_by_scan.next()?);
            assert_eq!(group_by_scan.get_i32("A")?, i);
            assert_eq!(group_by_scan.get_i32("B")?, i * 5 + 4);
        }
        assert!(!group_by_scan.next()?);

        Ok(())
    }
}
