use super::Scan;

pub trait Expression<T> {
    fn evaluate(&self, scan: &mut dyn Scan) -> Result<T, anyhow::Error>;
}

pub struct ConstantExpression<T> {
    value: T,
}

impl<T: Clone> Expression<T> for ConstantExpression<T> {
    fn evaluate(&self, _scan: &mut dyn Scan) -> Result<T, anyhow::Error> {
        Ok(self.value.clone())
    }
}

pub struct I32FieldExpression {
    field_name: String,
}

impl Expression<i32> for I32FieldExpression {
    fn evaluate(&self, scan: &mut dyn Scan) -> Result<i32, anyhow::Error> {
        scan.get_i32(&self.field_name)
    }
}

pub struct StringFieldExpression {
    field_name: String,
}

impl Expression<String> for StringFieldExpression {
    fn evaluate(&self, scan: &mut dyn Scan) -> Result<String, anyhow::Error> {
        scan.get_string(&self.field_name)
    }
}

pub trait Term {
    fn is_satisfied(&self, scan: &mut dyn Scan) -> Result<bool, anyhow::Error>;
}

pub struct EqualityTerm<T> {
    lhs: Box<dyn Expression<T>>,
    rhs: Box<dyn Expression<T>>,
}

impl<T: Clone + Eq + PartialEq> Term for EqualityTerm<T> {
    fn is_satisfied(&self, scan: &mut dyn Scan) -> Result<bool, anyhow::Error> {
        let lhs = self.lhs.evaluate(scan)?;
        let rhs = self.rhs.evaluate(scan)?;
        Ok(lhs == rhs)
    }
}

pub struct Predicate {
    terms: Vec<Box<dyn Term>>,
}

impl Predicate {
    pub fn new(terms: Vec<Box<dyn Term>>) -> Self {
        Predicate { terms }
    }

    fn is_satisfied(&self, scan: &mut dyn Scan) -> Result<bool, anyhow::Error> {
        for term in &self.terms {
            if !term.is_satisfied(scan)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

pub struct SelectScan<'a, T: Scan> {
    base_scan: &'a mut T,
    predicate: Predicate,
}

impl<'a, T: Scan> SelectScan<'a, T> {
    pub fn new(base_scan: &'a mut T, predicate: Predicate) -> Self {
        SelectScan {
            base_scan,
            predicate,
        }
    }
}

impl<'a, T: Scan> Scan for SelectScan<'a, T> {
    fn before_first(&mut self) -> Result<(), anyhow::Error> {
        self.base_scan.before_first()
    }

    fn next(&mut self) -> Result<bool, anyhow::Error> {
        while self.base_scan.next()? {
            if self.predicate.is_satisfied(&mut *self.base_scan)? {
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
        let layout = Layout::new(schema);

        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;

        let mut tx = db.new_transaction()?;

        let mut table_scan = TableScan::new(&mut tx, "testtable", &layout)?;
        table_scan.before_first()?;
        for i in 0..50 {
            table_scan.insert()?;
            table_scan.set_i32("A", i % 3)?;
            table_scan.set_string("B", &(i % 4).to_string())?;
            table_scan.set_i32("C", i)?;
        }

        let mut select_scan = SelectScan::new(
            &mut table_scan,
            Predicate::new(vec![
                Box::new(EqualityTerm {
                    lhs: Box::new(I32FieldExpression {
                        field_name: "A".to_string(),
                    }),
                    rhs: Box::new(ConstantExpression { value: 1 }),
                }),
                Box::new(EqualityTerm {
                    lhs: Box::new(StringFieldExpression {
                        field_name: "B".to_string(),
                    }),
                    rhs: Box::new(ConstantExpression {
                        value: "2".to_string(),
                    }),
                }),
            ]),
        );
        select_scan.before_first()?;
        for i in 0..50 {
            if i % 3 == 1 && i % 4 == 2 {
                dbg!(i);
                assert!(select_scan.next()?);
                assert_eq!(select_scan.get_i32("A")?, 1);
                assert_eq!(select_scan.get_string("B")?, "2");
                assert_eq!(select_scan.get_i32("C")?, i);
                select_scan.set_i32("C", i * 2)?;
            }
        }
        assert!(!select_scan.next()?);

        drop(select_scan);

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
        tx.commit()?;
        Ok(())
    }
}
