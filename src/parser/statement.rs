use crate::record::field::Spec;

use super::predicate::{Expression, Predicate};

#[derive(Debug, PartialEq, Eq)]
pub enum Statement {
    Query(QueryData),
    UpdateCommand(UpdateCommand),
}

#[derive(Debug, PartialEq, Eq)]
pub struct QueryData {
    pub fields: Vec<String>,
    pub tables: Vec<String>,
    pub predicate: Option<Predicate>,
}

impl QueryData {
    pub fn new(fields: Vec<String>, tables: Vec<String>, predicate: Option<Predicate>) -> Self {
        QueryData {
            fields,
            tables,
            predicate,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum UpdateCommand {
    Insert(String, Vec<String>, Vec<Constant>),
    Delete(String, Option<Predicate>),
    Modify(String, String, Expression, Option<Predicate>),
    Create(CreateCommand),
}

#[derive(Debug, PartialEq, Eq)]
pub enum CreateCommand {
    Table(String, Vec<FieldDefinition>),
    View(String, QueryData),
    Index(String, String, String),
}

#[derive(Debug, PartialEq, Eq)]
pub struct FieldDefinition {
    name: String,
    field_type: Spec,
}

impl FieldDefinition {
    pub fn new(name: String, field_type: Spec) -> Self {
        FieldDefinition { name, field_type }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Constant {
    I32(i32),
    String(String),
}
