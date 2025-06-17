pub mod grammar;
pub mod predicate;
pub mod statement;

#[cfg(test)]
mod tests {
    use crate::{
        parser::{
            grammar,
            predicate::{Expression, Predicate, Term},
            statement::{CreateCommand, FieldDefinition, QueryData, Statement, UpdateCommand},
        },
        record::field::{Spec, Value},
    };

    #[test]
    fn test_predicate() {
        assert_eq!(
            grammar::TermParser::new().parse("333 = 222").unwrap(),
            Term::Equality(Expression::I32Constant(333), Expression::I32Constant(222))
        );
        assert_eq!(
            grammar::PredicateParser::new()
                .parse("col = 222 AND string = '222'")
                .unwrap(),
            Predicate::new(vec![
                Term::Equality(
                    Expression::Field("col".to_string()),
                    Expression::I32Constant(222)
                ),
                Term::Equality(
                    Expression::Field("string".to_string()),
                    Expression::StringConstant("222".to_string())
                ),
            ])
        );

        assert_eq!(
            grammar::TermParser::new().parse("NULL = NULL").unwrap(),
            Term::Equality(Expression::NullConstant, Expression::NullConstant)
        );
    }

    #[test]
    fn test_insert() {
        assert_eq!(
            grammar::StatementParser::new()
                .parse("INSERT INTO mytable (aaa, bbb) VALUES (333, '222')")
                .unwrap(),
            Statement::UpdateCommand(UpdateCommand::Insert(
                "mytable".to_string(),
                vec!["aaa".to_string(), "bbb".to_string()],
                vec![Value::I32(333), Value::String("222".to_string())]
            ))
        );
    }

    #[test]
    fn test_delete() {
        assert_eq!(
            grammar::StatementParser::new()
                .parse("DELETE FROM mytable")
                .unwrap(),
            Statement::UpdateCommand(UpdateCommand::Delete("mytable".to_string(), None))
        );

        assert_eq!(
            grammar::StatementParser::new()
                .parse("DELETE FROM mytable WHERE col = 222")
                .unwrap(),
            Statement::UpdateCommand(UpdateCommand::Delete(
                "mytable".to_string(),
                Some(Predicate::new(vec![Term::Equality(
                    Expression::Field("col".to_string()),
                    Expression::I32Constant(222)
                )]))
            ))
        );
    }

    #[test]
    fn test_modify() {
        assert_eq!(
            grammar::StatementParser::new()
                .parse("MODIFY mytable SET aaa = 333")
                .unwrap(),
            Statement::UpdateCommand(UpdateCommand::Modify(
                "mytable".to_string(),
                "aaa".to_string(),
                Expression::I32Constant(333),
                None
            ))
        );

        assert_eq!(
            grammar::StatementParser::new()
                .parse("MODIFY mytable SET aaa = 333 WHERE col = 222")
                .unwrap(),
            Statement::UpdateCommand(UpdateCommand::Modify(
                "mytable".to_string(),
                "aaa".to_string(),
                Expression::I32Constant(333),
                Some(Predicate::new(vec![Term::Equality(
                    Expression::Field("col".to_string()),
                    Expression::I32Constant(222)
                )]))
            ))
        );
    }

    #[test]
    fn test_create_index() {
        assert_eq!(
            grammar::StatementParser::new()
                .parse("CREATE INDEX index_name ON table_name (field_name)")
                .unwrap(),
            Statement::UpdateCommand(UpdateCommand::Create(CreateCommand::Index(
                "index_name".to_string(),
                "table_name".to_string(),
                "field_name".to_string()
            )))
        );
    }

    #[test]
    fn test_create_view() {
        assert_eq!(
            grammar::StatementParser::new()
                .parse("CREATE VIEW view_name AS SELECT aaa, bbb FROM table_name WHERE col = 222")
                .unwrap(),
            Statement::UpdateCommand(UpdateCommand::Create(CreateCommand::View(
                "view_name".to_string(),
                QueryData::new(
                    vec!["aaa".to_string(), "bbb".to_string()],
                    vec!["table_name".to_string()],
                    Some(Predicate::new(vec![Term::Equality(
                        Expression::Field("col".to_string()),
                        Expression::I32Constant(222)
                    )]))
                )
            )))
        );
    }

    #[test]
    fn test_create_table() {
        assert_eq!(
            grammar::StatementParser::new()
                .parse("CREATE TABLE table_name (aaa I32, bbb VARCHAR(20))")
                .unwrap(),
            Statement::UpdateCommand(UpdateCommand::Create(CreateCommand::Table(
                "table_name".to_string(),
                vec![
                    FieldDefinition::new("aaa".to_string(), Spec::I32),
                    FieldDefinition::new("bbb".to_string(), Spec::VarChar(20))
                ]
            )))
        );
    }

    #[test]
    fn test_select_all() {
        assert_eq!(
            grammar::StatementParser::new()
                .parse("SELECT * FROM mytable")
                .unwrap(),
            Statement::Query(QueryData::new_all(vec!["mytable".to_string()], None))
        );
    }
}
