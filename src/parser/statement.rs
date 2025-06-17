use crate::record::field::{Spec, Value};

use super::predicate::{Expression, Predicate};

#[derive(Debug, PartialEq, Eq)]
pub enum Statement {
    Query(QueryData),
    UpdateCommand(UpdateCommand),
}

#[derive(Debug, PartialEq, Eq)]
pub struct QueryData {
    pub fields: Option<Vec<String>>, // None means all fields
    pub tables: Vec<String>,
    pub predicate: Option<Predicate>,
}

impl QueryData {
    pub fn new(fields: Vec<String>, tables: Vec<String>, predicate: Option<Predicate>) -> Self {
        QueryData {
            fields: Some(fields),
            tables,
            predicate,
        }
    }

    pub fn new_all(tables: Vec<String>, predicate: Option<Predicate>) -> Self {
        QueryData {
            fields: None,
            tables,
            predicate,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum UpdateCommand {
    Insert(String, Vec<String>, Vec<Value>),
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
    pub name: String,
    pub field_type: Spec,
}

impl FieldDefinition {
    pub fn new(name: String, field_type: Spec) -> Self {
        FieldDefinition { name, field_type }
    }
}
