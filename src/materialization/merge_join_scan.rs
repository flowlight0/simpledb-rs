use crate::{errors::TransactionError, record::field::Value, scan::ScanControl};

use super::sort_scan::SortScan;

pub struct MergeJoinScan {
    s1: SortScan,
    s2: SortScan,
    field_name1: String,
    field_name2: String,
    join_value: Option<Value>,
}

impl MergeJoinScan {
    pub fn new(
        s1: SortScan,
        s2: SortScan,
        field_name1: &str,
        field_name2: &str,
    ) -> Result<Self, TransactionError> {
        let mut scan = MergeJoinScan {
            s1,
            s2,
            field_name1: field_name1.to_string(),
            field_name2: field_name2.to_string(),
            join_value: None,
        };
        scan.before_first()?;
        Ok(scan)
    }
}

impl ScanControl for MergeJoinScan {
    fn before_first(&mut self) -> Result<(), TransactionError> {
        self.s1.before_first()?;
        self.s2.before_first()
    }

    fn next(&mut self) -> Result<bool, TransactionError> {
        let mut has_next2 = self.s2.next()?;
        if has_next2 && Some(self.s2.get_value(&self.field_name2)?) == self.join_value {
            return Ok(true);
        }

        let mut has_next1 = self.s1.next()?;
        if has_next1 && Some(self.s1.get_value(&self.field_name1)?) == self.join_value {
            self.s2.restore_position()?;
            return Ok(true);
        }

        while has_next1 && has_next2 {
            let value1 = self.s1.get_value(&self.field_name1)?;
            let value2 = self.s2.get_value(&self.field_name2)?;
            match value1.cmp(&value2) {
                std::cmp::Ordering::Less => {
                    has_next1 = self.s1.next()?;
                }
                std::cmp::Ordering::Greater => {
                    has_next2 = self.s2.next()?;
                }
                std::cmp::Ordering::Equal => {
                    self.join_value = Some(value1);
                    self.s2.save_position();
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn get_i32(&mut self, field_name: &str) -> Result<i32, TransactionError> {
        if self.s1.has_field(field_name) {
            self.s1.get_i32(field_name)
        } else {
            self.s2.get_i32(field_name)
        }
    }

    fn get_string(&mut self, field_name: &str) -> Result<String, TransactionError> {
        if self.s1.has_field(field_name) {
            self.s1.get_string(field_name)
        } else {
            self.s2.get_string(field_name)
        }
    }

    fn get_value(&mut self, field_name: &str) -> Result<Value, TransactionError> {
        if self.s1.has_field(field_name) {
            self.s1.get_value(field_name)
        } else {
            self.s2.get_value(field_name)
        }
    }

    fn has_field(&self, field_name: &str) -> bool {
        self.s1.has_field(field_name) || self.s2.has_field(field_name)
    }
}

#[cfg(test)]
mod tests {}
