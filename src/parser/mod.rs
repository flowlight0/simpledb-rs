pub mod expression;
pub mod grammar;
pub mod predicate;
pub mod statement;

#[cfg(test)]
mod tests {
    use crate::{
        materialization::{aggregation_function::AggregationFn, sum_function::SumFn},
        parser::{
            expression::Expression,
            grammar,
            predicate::{Predicate, Term},
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

        assert_eq!(
            grammar::TermParser::new().parse("f IS NULL").unwrap(),
            Term::IsNull(Expression::Field("f".to_string()))
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

    #[test]
    fn test_select_order_by() {
        assert_eq!(
            grammar::StatementParser::new()
                .parse("SELECT * FROM mytable ORDER BY col")
                .unwrap(),
            Statement::Query(QueryData::new_all_with_order(
                vec!["mytable".to_string()],
                None,
                Some(vec!["col".to_string()]),
            ))
        );
    }

    #[test]
    fn test_expression_arithmetic() {
        assert_eq!(
            grammar::TermParser::new().parse("1 + 2 * 3 = 7").unwrap(),
            Term::Equality(
                Expression::Add(
                    Box::new(Expression::I32Constant(1)),
                    Box::new(Expression::Mul(
                        Box::new(Expression::I32Constant(2)),
                        Box::new(Expression::I32Constant(3)),
                    )),
                ),
                Expression::I32Constant(7),
            )
        );

        assert_eq!(
            grammar::TermParser::new().parse("(a + 1) / b = 3").unwrap(),
            Term::Equality(
                Expression::Div(
                    Box::new(Expression::Add(
                        Box::new(Expression::Field("a".to_string())),
                        Box::new(Expression::I32Constant(1)),
                    )),
                    Box::new(Expression::Field("b".to_string())),
                ),
                Expression::I32Constant(3),
            )
        );
    }

    #[test]
    fn test_select_as_alias() {
        assert_eq!(
            grammar::StatementParser::new()
                .parse("SELECT col AS alias FROM tbl")
                .unwrap(),
            Statement::Query(QueryData::new_with_order_and_extend(
                vec!["alias".to_string()],
                vec!["tbl".to_string()],
                None,
                None,
                vec![(Expression::Field("col".to_string()), "alias".to_string())],
            ))
        );
    }

    #[test]
    fn test_select_expression_as_alias() {
        assert_eq!(
            grammar::StatementParser::new()
                .parse("SELECT a + 1 AS b FROM tbl")
                .unwrap(),
            Statement::Query(QueryData::new_with_order_and_extend(
                vec!["b".to_string()],
                vec!["tbl".to_string()],
                None,
                None,
                vec![(
                    Expression::Add(
                        Box::new(Expression::Field("a".to_string())),
                        Box::new(Expression::I32Constant(1)),
                    ),
                    "b".to_string(),
                ),],
            ))
        );
    }

    #[test]
    fn test_select_aggregation_as_alias() {
        assert_eq!(
            grammar::StatementParser::new()
                .parse("SELECT SUM(val) AS total FROM t GROUP BY val")
                .unwrap(),
            Statement::Query(QueryData::new_full(
                Some(vec!["total".to_string()]),
                vec!["t".to_string()],
                None,
                Some(vec!["val".to_string()]),
                None,
                vec![(Expression::Field("val".to_string()), "total".to_string())],
                vec![AggregationFn::from(SumFn::new("val"))],
            ))
        );
    }
}
