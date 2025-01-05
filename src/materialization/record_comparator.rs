use std::cmp::Ordering;

use crate::scan::{Scan, ScanControl};

pub struct RecordComparator {
    field_names: Vec<String>,
}

impl RecordComparator {
    pub fn new(field_names: &Vec<String>) -> Self {
        RecordComparator {
            field_names: field_names.clone(),
        }
    }

    pub fn compare(&self, s1: &mut Scan, s2: &mut Scan) -> Ordering {
        for field_name in &self.field_names {
            let v1 = s1.get_value(field_name).unwrap();
            let v2 = s2.get_value(field_name).unwrap();
            let cmp = v1.cmp(&v2);
            if cmp != std::cmp::Ordering::Equal {
                return cmp;
            }
        }
        Ordering::Equal
    }
}
