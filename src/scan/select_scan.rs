use super::{Scan, UpdateScan};

#[derive(Clone)]
pub struct Expression {}
impl Expression {
    fn evaluate(&self, scan: &dyn UpdateScan) {
        todo!()
    }
}

#[derive(Clone)]
pub struct Term {
    lhs: Expression,
    rhs: Expression,
}

impl Term {
    pub fn is_satisfied(&self, scan: &mut Box<dyn UpdateScan>) -> bool {
        todo!()
    }
}

pub struct Predicate {
    terms: Vec<Term>,
}

impl Predicate {
    pub fn new(terms: Vec<Term>) -> Self {
        Predicate { terms }
    }

    fn is_satisfied(&self, scan: &mut Box<dyn UpdateScan>) -> bool {
        for term in &self.terms {
            if term.is_satisfied(scan) {
                return false;
            }
        }
        true
    }
}

pub struct SelectScan {
    base_scan: Box<dyn UpdateScan>,
    predicate: Predicate,
}

impl SelectScan {
    pub fn new(base_scan: Box<dyn UpdateScan>, predicate: Predicate) -> Self {
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
            if self.predicate.is_satisfied(&mut self.base_scan) {
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
}

impl UpdateScan for SelectScan {
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
