mod ast;
mod grammar;

#[cfg(test)]
mod tests {
    use crate::{
        parser::{
            ast::{
                Constant, CreateCommand, Expression, FieldDefinition, Predicate, QueryCommand,
                Statement, Term, UpdateCommand,
            },
            grammar,
        },
        record::field::Type,
    };

    #[test]
    fn test_predicate() {
        assert_eq!(
            grammar::TermParser::new().parse("333 = 222").unwrap(),
            Term::EqualityTerm(Expression::I32Constant(333), Expression::I32Constant(222))
        );
        assert_eq!(
            grammar::PredicateParser::new()
                .parse("i32 = 222 AND string = '222'")
                .unwrap(),
            Predicate::new(vec![
                Term::EqualityTerm(
                    Expression::FieldExpression("i32".to_string()),
                    Expression::I32Constant(222)
                ),
                Term::EqualityTerm(
                    Expression::FieldExpression("string".to_string()),
                    Expression::StringConstant("222".to_string())
                ),
            ])
        );
    }

    #[test]
    fn test_insert() {
        assert_eq!(
            grammar::StatementParser::new()
                .parse("INSERT INTO table (aaa, bbb) VALUES (333, '222')")
                .unwrap(),
            Statement::UpdateCommand(UpdateCommand::Insert(
                "table".to_string(),
                vec!["aaa".to_string(), "bbb".to_string()],
                vec![Constant::I32(333), Constant::String("222".to_string())]
            ))
        );
    }

    #[test]
    fn test_delete() {
        assert_eq!(
            grammar::StatementParser::new()
                .parse("DELETE FROM table")
                .unwrap(),
            Statement::UpdateCommand(UpdateCommand::Delete("table".to_string(), None))
        );

        assert_eq!(
            grammar::StatementParser::new()
                .parse("DELETE FROM table WHERE i32 = 222")
                .unwrap(),
            Statement::UpdateCommand(UpdateCommand::Delete(
                "table".to_string(),
                Some(Predicate::new(vec![Term::EqualityTerm(
                    Expression::FieldExpression("i32".to_string()),
                    Expression::I32Constant(222)
                )]))
            ))
        );
    }

    #[test]
    fn test_modify() {
        assert_eq!(
            grammar::StatementParser::new()
                .parse("MODIFY table SET aaa = 333")
                .unwrap(),
            Statement::UpdateCommand(UpdateCommand::Modify(
                "table".to_string(),
                "aaa".to_string(),
                Expression::I32Constant(333),
                None
            ))
        );

        assert_eq!(
            grammar::StatementParser::new()
                .parse("MODIFY table SET aaa = 333 WHERE i32 = 222")
                .unwrap(),
            Statement::UpdateCommand(UpdateCommand::Modify(
                "table".to_string(),
                "aaa".to_string(),
                Expression::I32Constant(333),
                Some(Predicate::new(vec![Term::EqualityTerm(
                    Expression::FieldExpression("i32".to_string()),
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
                .parse("CREATE VIEW view_name AS SELECT aaa, bbb FROM table_name WHERE i32 = 222")
                .unwrap(),
            Statement::UpdateCommand(UpdateCommand::Create(CreateCommand::View(
                "view_name".to_string(),
                QueryCommand::new(
                    vec!["aaa".to_string(), "bbb".to_string()],
                    vec!["table_name".to_string()],
                    Some(Predicate::new(vec![Term::EqualityTerm(
                        Expression::FieldExpression("i32".to_string()),
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
                .parse("CREATE TABLE table_name (aaa INT, bbb VARCHAR(20))")
                .unwrap(),
            Statement::UpdateCommand(UpdateCommand::Create(CreateCommand::Table(
                "table_name".to_string(),
                vec![
                    FieldDefinition::new("aaa".to_string(), Type::I32),
                    FieldDefinition::new("bbb".to_string(), Type::VarChar(20))
                ]
            )))
        );
    }
}
