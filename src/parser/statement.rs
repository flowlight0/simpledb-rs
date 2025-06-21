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
    pub order_by: Option<Vec<String>>, // None means no ordering
    pub extend_fields: Vec<(Expression, String)>,
}

impl QueryData {
    pub fn new(fields: Vec<String>, tables: Vec<String>, predicate: Option<Predicate>) -> Self {
        QueryData::new_with_order_and_extend(fields, tables, predicate, None, vec![])
    }

    pub fn new_all(tables: Vec<String>, predicate: Option<Predicate>) -> Self {
        QueryData::new_all_with_order(tables, predicate, None)
    }

    pub fn new_with_order(
        fields: Vec<String>,
        tables: Vec<String>,
        predicate: Option<Predicate>,
        order_by: Option<Vec<String>>,
    ) -> Self {
        QueryData {
            fields: Some(fields),
            tables,
            predicate,
            order_by,
            extend_fields: Vec::new(),
        }
    }

    pub fn new_all_with_order(
        tables: Vec<String>,
        predicate: Option<Predicate>,
        order_by: Option<Vec<String>>,
    ) -> Self {
        QueryData {
            fields: None,
            tables,
            predicate,
            order_by,
            extend_fields: Vec::new(),
        }
    }

    pub fn new_with_order_and_extend(
        fields: Vec<String>,
        tables: Vec<String>,
        predicate: Option<Predicate>,
        order_by: Option<Vec<String>>,
        extend_fields: Vec<(Expression, String)>,
    ) -> Self {
        QueryData {
            fields: Some(fields),
            tables,
            predicate,
            order_by,
            extend_fields,
        }
    }

    pub fn new_all_with_order_and_extend(
        tables: Vec<String>,
        predicate: Option<Predicate>,
        order_by: Option<Vec<String>>,
        extend_fields: Vec<(Expression, String)>,
    ) -> Self {
        QueryData {
            fields: None,
            tables,
            predicate,
            order_by,
            extend_fields,
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
