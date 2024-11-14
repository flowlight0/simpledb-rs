use crate::{plan::Plan, record::field::Value};

use super::Scan;

#[derive(Clone)]
pub enum Expression {
    I32Constant(i32),
    StringConstant(String),
    Field(String),
}

impl Expression {
    pub fn evaluate(&self, scan: &mut Box<dyn Scan>) -> Result<Value, anyhow::Error> {
        match self {
            Expression::I32Constant(value) => Ok(Value::I32(*value)),
            Expression::StringConstant(value) => Ok(Value::String(value.clone())),
            Expression::Field(field_name) => {
                if scan.has_field(field_name) {
                    scan.get_value(field_name)
                } else {
                    Err(anyhow::anyhow!("Field {} not found", field_name))
                }
            }
        }
    }
}

#[derive(Clone)]
pub enum Term {
    Equality(Expression, Expression),
}

impl Term {
    pub fn is_satisfied(&self, scan: &mut Box<dyn Scan>) -> Result<bool, anyhow::Error> {
        match self {
            Term::Equality(lhs, rhs) => {
                let lhs = lhs.evaluate(scan)?;
                let rhs = rhs.evaluate(scan)?;
                Ok(lhs == rhs)
            }
        }
    }

    pub fn get_reduction_factor(&self, plan: &Box<dyn Plan>) -> usize {
        todo!()
    }
}

#[derive(Clone)]
pub struct Predicate {
    terms: Vec<Term>,
}

impl Predicate {
    pub fn new(terms: Vec<Term>) -> Self {
        Predicate { terms }
    }

    fn is_satisfied(&self, scan: &mut Box<dyn Scan>) -> Result<bool, anyhow::Error> {
        for term in &self.terms {
            if !term.is_satisfied(scan)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(crate) fn get_reduction_factor(&self, plan: &Box<dyn Plan>) -> usize {
        let mut factor = 1;
        for term in &self.terms {
            factor *= term.get_reduction_factor(plan);
        }
        factor
    }
}

pub struct SelectScan {
    base_scan: Box<dyn Scan>,
    predicate: Predicate,
}

impl SelectScan {
    pub fn new(base_scan: Box<dyn Scan>, predicate: Predicate) -> Self {
        SelectScan {
            base_scan,
            predicate,
        }
    }
}

impl Scan for SelectScan {
    fn before_first(&mut self) -> Result<(), anyhow::Error> {
        self.base_scan.before_first()
    }

    fn next(&mut self) -> Result<bool, anyhow::Error> {
        while self.base_scan.next()? {
            if self.predicate.is_satisfied(&mut self.base_scan)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn get_i32(&mut self, field_name: &str) -> Result<i32, anyhow::Error> {
        self.base_scan.get_i32(field_name)
    }

    fn get_string(&mut self, field_name: &str) -> Result<String, anyhow::Error> {
        self.base_scan.get_string(field_name)
    }

    fn get_value(&mut self, field_name: &str) -> Result<Value, anyhow::Error> {
        self.base_scan.get_value(field_name)
    }

    fn has_field(&self, field_name: &str) -> bool {
        self.base_scan.has_field(field_name)
    }

    fn set_i32(&mut self, field_name: &str, value: i32) -> Result<(), anyhow::Error> {
        self.base_scan.set_i32(field_name, value)
    }

    fn set_string(&mut self, field_name: &str, value: &str) -> Result<(), anyhow::Error> {
        self.base_scan.set_string(field_name, value)
    }

    fn delete(&mut self) -> Result<(), anyhow::Error> {
        self.base_scan.delete()
    }

    fn insert(&mut self) -> Result<(), anyhow::Error> {
        self.base_scan.insert()
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::db::SimpleDB;
    use crate::record::layout::Layout;
    use crate::record::schema::Schema;
    use crate::scan::table_scan::TableScan;

    #[test]
    fn test_select_scan() -> Result<(), anyhow::Error> {
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);
        schema.add_i32_field("C");
        let layout = Rc::new(Layout::new(schema));

        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        let mut table_scan: Box<dyn Scan> =
            Box::new(TableScan::new(tx.clone(), "testtable", layout.clone())?);
        table_scan.before_first()?;
        for i in 0..50 {
            table_scan.insert()?;
            table_scan.set_i32("A", i % 3)?;
            table_scan.set_string("B", &(i % 4).to_string())?;
            table_scan.set_i32("C", i)?;
        }

        let mut select_scan = SelectScan::new(
            table_scan,
            Predicate::new(vec![
                Term::Equality(
                    Expression::Field("A".to_string()),
                    Expression::I32Constant(1),
                ),
                Term::Equality(
                    Expression::Field("B".to_string()),
                    Expression::StringConstant("2".to_string()),
                ),
            ]),
        );
        select_scan.before_first()?;
        for i in 0..50 {
            if i % 3 == 1 && i % 4 == 2 {
                assert!(select_scan.next()?);
                assert_eq!(select_scan.get_i32("A")?, 1);
                assert_eq!(select_scan.get_string("B")?, "2");
                assert_eq!(select_scan.get_i32("C")?, i);
                select_scan.set_i32("C", i * 2)?;
            }
        }
        assert!(!select_scan.next()?);
        drop(select_scan);

        let mut table_scan = TableScan::new(tx.clone(), "testtable", layout.clone())?;

        table_scan.before_first()?;
        for i in 0..50 {
            table_scan.next()?;
            if i % 3 == 1 && i % 4 == 2 {
                assert_eq!(table_scan.get_i32("C")?, i * 2);
            } else {
                assert_eq!(table_scan.get_i32("C")?, i);
            }
        }
        drop(table_scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }
}
