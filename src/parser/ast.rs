use crate::record::field::Spec;

#[derive(Debug, PartialEq, Eq)]
pub enum Statement {
    Query(QueryCommand),
    UpdateCommand(UpdateCommand),
}

#[derive(Debug, PartialEq, Eq)]
pub struct QueryCommand {
    fields: Vec<String>,
    tables: Vec<String>,
    predicate: Option<Predicate>,
}

impl QueryCommand {
    pub fn new(fields: Vec<String>, tables: Vec<String>, predicate: Option<Predicate>) -> Self {
        QueryCommand {
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
    View(String, QueryCommand),
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
pub struct Predicate {
    terms: Vec<Term>,
}

impl Predicate {
    pub fn new(terms: Vec<Term>) -> Self {
        Predicate { terms }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Term {
    EqualityTerm(Expression, Expression),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Expression {
    I32Constant(i32),
    StringConstant(String),
    FieldExpression(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Constant {
    I32(i32),
    String(String),
}
