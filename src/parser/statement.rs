use crate::record::field::{Spec, Value};

use super::{expression::Expression, predicate::Predicate};
use crate::materialization::aggregation_function::AggregationFn;

#[derive(Debug, PartialEq, Eq)]
pub enum Statement {
    Query(QueryData),
    UpdateCommand(UpdateCommand),
}

#[derive(Debug, PartialEq, Eq)]
pub enum SelectField {
    Expression(Expression, Option<String>),
    Aggregation(AggregationFn),
}

#[derive(Debug, PartialEq, Eq)]
pub struct QueryData {
    pub fields: Option<Vec<String>>, // None means all fields
    pub tables: Vec<String>,
    pub predicate: Option<Predicate>,
    pub group_by: Option<Vec<String>>, // None means no grouping
    pub order_by: Option<Vec<String>>, // None means no ordering
    pub extend_fields: Vec<(Expression, String)>,
    pub aggregation_functions: Vec<AggregationFn>,
}

impl QueryData {
    pub fn new_full(
        fields: Option<Vec<String>>,
        tables: Vec<String>,
        predicate: Option<Predicate>,
        group_by: Option<Vec<String>>,
        order_by: Option<Vec<String>>,
        extend_fields: Vec<(Expression, String)>,
        aggregation_functions: Vec<AggregationFn>,
    ) -> Self {
        QueryData {
            fields,
            tables,
            predicate,
            group_by,
            order_by,
            extend_fields,
            aggregation_functions,
        }
    }

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
        QueryData::new_full(
            Some(fields),
            tables,
            predicate,
            None,
            order_by,
            Vec::new(),
            Vec::new(),
        )
    }

    pub fn new_all_with_order(
        tables: Vec<String>,
        predicate: Option<Predicate>,
        order_by: Option<Vec<String>>,
    ) -> Self {
        QueryData::new_full(
            None,
            tables,
            predicate,
            None,
            order_by,
            Vec::new(),
            Vec::new(),
        )
    }

    pub fn new_with_order_and_extend(
        fields: Vec<String>,
        tables: Vec<String>,
        predicate: Option<Predicate>,
        order_by: Option<Vec<String>>,
        extend_fields: Vec<(Expression, String)>,
    ) -> Self {
        QueryData::new_full(
            Some(fields),
            tables,
            predicate,
            None,
            order_by,
            extend_fields,
            Vec::new(),
        )
    }

    pub fn new_all_with_order_and_extend(
        tables: Vec<String>,
        predicate: Option<Predicate>,
        order_by: Option<Vec<String>>,
        extend_fields: Vec<(Expression, String)>,
    ) -> Self {
        QueryData::new_full(
            None,
            tables,
            predicate,
            None,
            order_by,
            extend_fields,
            Vec::new(),
        )
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
