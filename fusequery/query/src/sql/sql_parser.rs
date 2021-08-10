// Copyright 2020-2021 The Datafuse Authors.
//
// SPDX-License-Identifier: Apache-2.0.
//
// Borrow from apache/arrow/rust/datafusion/src/sql/sql_parser
// See notice.md

use common_exception::ErrorCode;
use common_planners::DatabaseEngineType;
use common_planners::ExplainType;
use common_planners::TableEngineType;
use sqlparser::ast::ColumnDef;
use sqlparser::ast::ColumnOptionDef;
use sqlparser::ast::Ident;
use sqlparser::ast::SqlOption;
use sqlparser::ast::TableConstraint;
use sqlparser::ast::Value;
use sqlparser::dialect::keywords::Keyword;
use sqlparser::dialect::Dialect;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
use sqlparser::parser::ParserError;
use sqlparser::tokenizer::Token;
use sqlparser::tokenizer::Tokenizer;
use sqlparser::tokenizer::Whitespace;

use crate::sql::DfCreateDatabase;
use crate::sql::DfCreateTable;
use crate::sql::DfDescribeTable;
use crate::sql::DfDropDatabase;
use crate::sql::DfDropTable;
use crate::sql::DfExplain;
use crate::sql::DfHint;
use crate::sql::DfKillStatement;
use crate::sql::DfShowCreateTable;
use crate::sql::DfShowDatabases;
use crate::sql::DfShowProcessList;
use crate::sql::DfShowSettings;
use crate::sql::DfShowTables;
use crate::sql::DfStatement;
use crate::sql::DfTruncateTable;
use crate::sql::DfUseDatabase;

// Use `Parser::expected` instead, if possible
macro_rules! parser_err {
    ($MSG:expr) => {
        Err(ParserError::ParserError($MSG.to_string().into()))
    };
}

/// SQL Parser
pub struct DfParser<'a> {
    parser: Parser<'a>,
}

impl<'a> DfParser<'a> {
    /// Parse the specified tokens
    pub fn new(sql: &str) -> Result<Self, ParserError> {
        let dialect = &GenericDialect {};
        DfParser::new_with_dialect(sql, dialect)
    }

    /// Parse the specified tokens with dialect
    pub fn new_with_dialect(sql: &str, dialect: &'a dyn Dialect) -> Result<Self, ParserError> {
        let mut tokenizer = Tokenizer::new(dialect, sql);
        let tokens = tokenizer.tokenize()?;

        Ok(DfParser {
            parser: Parser::new(tokens, dialect),
        })
    }

    /// Parse a SQL statement and produce a set of statements with dialect
    pub fn parse_sql(sql: &str) -> Result<(Vec<DfStatement>, Vec<DfHint>), ErrorCode> {
        let dialect = &GenericDialect {};
        Ok(DfParser::parse_sql_with_dialect(sql, dialect)?)
    }

    /// Parse a SQL statement and produce a set of statements
    pub fn parse_sql_with_dialect(
        sql: &str,
        dialect: &dyn Dialect,
    ) -> Result<(Vec<DfStatement>, Vec<DfHint>), ParserError> {
        let mut parser = DfParser::new_with_dialect(sql, dialect)?;
        let mut stmts = Vec::new();

        let mut expecting_statement_delimiter = false;
        loop {
            // ignore empty statements (between successive statement delimiters)
            while parser.parser.consume_token(&Token::SemiColon) {
                expecting_statement_delimiter = false;
            }

            if parser.parser.peek_token() == Token::EOF {
                break;
            }
            if expecting_statement_delimiter {
                return parser.expected("end of statement", parser.parser.peek_token());
            }

            let statement = parser.parse_statement()?;
            stmts.push(statement);
            expecting_statement_delimiter = true;
        }

        let mut hints = Vec::new();

        let mut parser = DfParser::new_with_dialect(sql, dialect)?;
        loop {
            let token = parser.parser.next_token_no_skip();
            match token {
                Some(Token::Whitespace(Whitespace::SingleLineComment { comment, prefix })) => {
                    hints.push(DfHint::create_from_comment(comment, prefix));
                }
                Some(Token::Whitespace(Whitespace::Newline)) | Some(Token::EOF) | None => break,
                _ => continue,
            }
        }
        Ok((stmts, hints))
    }

    /// Report unexpected token
    fn expected<T>(&self, expected: &str, found: Token) -> Result<T, ParserError> {
        parser_err!(format!("Expected {}, found: {}", expected, found))
    }

    /// Parse a new expression
    pub fn parse_statement(&mut self) -> Result<DfStatement, ParserError> {
        match self.parser.peek_token() {
            Token::Word(w) => {
                match w.keyword {
                    Keyword::CREATE => {
                        self.parser.next_token();
                        self.parse_create()
                    }
                    Keyword::DESC => {
                        self.parser.next_token();
                        self.parse_describe()
                    }
                    Keyword::DESCRIBE => {
                        self.parser.next_token();
                        self.parse_describe()
                    }
                    Keyword::DROP => {
                        self.parser.next_token();
                        self.parse_drop()
                    }
                    Keyword::EXPLAIN => {
                        self.parser.next_token();
                        self.parse_explain()
                    }
                    Keyword::SHOW => {
                        log::debug!("show first");
                        self.parser.next_token();

                        if self.consume_token("TABLES") {
                            // self.parser.next_token();
                            // let token = self.parser.peek_token();

                            let db_name = if self.parser.parse_keyword(Keyword::FROM) {
                                let db_name = self.parser.peek_token().to_string();
                                self.parser.next_token();

                                Some(db_name)
                            }else {
                                None 
                            };

                            log::debug!("db name:{:?}", db_name);
                            Ok(DfStatement::ShowTables(DfShowTables{db_name}))
                        } else if self.consume_token("DATABASES") {
                            Ok(DfStatement::ShowDatabases(DfShowDatabases))
                        } else if self.consume_token("SETTINGS") {
                            Ok(DfStatement::ShowSettings(DfShowSettings))
                        } else if self.consume_token("CREATE") {
                            self.parse_show_create()
                        } else if self.consume_token("PROCESSLIST") {
                            Ok(DfStatement::ShowProcessList(DfShowProcessList))
                        } else {
                            self.expected("tables or settings", self.parser.peek_token())
                        }
                    }
                    Keyword::TRUNCATE => {
                        self.parser.next_token();
                        self.parse_truncate()
                    }
                    Keyword::NoKeyword => match w.value.to_uppercase().as_str() {
                        // Use database
                        "USE" => self.parse_use_database(),
                        "KILL" => self.parse_kill_query(),
                        _ => self.expected("Keyword", self.parser.peek_token()),
                    },
                    _ => {
                        // use the native parser
                        Ok(DfStatement::Statement(self.parser.parse_statement()?))
                    }
                }
            }
            a => {
                log::debug!("show second, {:?}", a);
                // use the native parser
                Ok(DfStatement::Statement(self.parser.parse_statement()?))
            }
        }
    }

    /// Parse an SQL EXPLAIN statement.
    pub fn parse_explain(&mut self) -> Result<DfStatement, ParserError> {
        // Parser is at the token immediately after EXPLAIN
        // Check for EXPLAIN VERBOSE
        let typ = match self.parser.peek_token() {
            Token::Word(w) => match w.value.to_uppercase().as_str() {
                "PIPELINE" => {
                    self.parser.next_token();
                    ExplainType::Pipeline
                }
                "GRAPH" => {
                    self.parser.next_token();
                    ExplainType::Graph
                }
                _ => ExplainType::Syntax,
            },
            _ => ExplainType::Syntax,
        };

        let statement = Box::new(self.parser.parse_statement()?);
        let explain_plan = DfExplain { typ, statement };
        Ok(DfStatement::Explain(explain_plan))
    }

    // This is a copy of the equivalent implementation in sqlparser.
    fn parse_columns(&mut self) -> Result<(Vec<ColumnDef>, Vec<TableConstraint>), ParserError> {
        let mut columns = vec![];
        let mut constraints = vec![];
        if !self.parser.consume_token(&Token::LParen) || self.parser.consume_token(&Token::RParen) {
            return Ok((columns, constraints));
        }

        loop {
            if let Some(constraint) = self.parser.parse_optional_table_constraint()? {
                constraints.push(constraint);
            } else if let Token::Word(_) = self.parser.peek_token() {
                let column_def = self.parse_column_def()?;
                columns.push(column_def);
            } else {
                return self.expected(
                    "column name or constraint definition",
                    self.parser.peek_token(),
                );
            }
            let comma = self.parser.consume_token(&Token::Comma);
            if self.parser.consume_token(&Token::RParen) {
                // allow a trailing comma, even though it's not in standard
                break;
            } else if !comma {
                return self.expected(
                    "',' or ')' after column definition",
                    self.parser.peek_token(),
                );
            }
        }

        Ok((columns, constraints))
    }

    /// This is a copy from sqlparser
    /// Parse a literal value (numbers, strings, date/time, booleans)
    fn parse_value(&mut self) -> Result<Value, ParserError> {
        match self.parser.next_token() {
            Token::Word(w) => match w.keyword {
                Keyword::TRUE => Ok(Value::Boolean(true)),
                Keyword::FALSE => Ok(Value::Boolean(false)),
                Keyword::NULL => Ok(Value::Null),
                Keyword::NoKeyword if w.quote_style.is_some() => match w.quote_style {
                    Some('"') => Ok(Value::DoubleQuotedString(w.value)),
                    Some('\'') => Ok(Value::SingleQuotedString(w.value)),
                    _ => self.expected("A value?", Token::Word(w))?,
                },
                _ => self.expected("a concrete value", Token::Word(w)),
            },
            // The call to n.parse() returns a bigdecimal when the
            // bigdecimal feature is enabled, and is otherwise a no-op
            // (i.e., it returns the input string).
            Token::Number(ref n, l) => match n.parse() {
                Ok(n) => Ok(Value::Number(n, l)),
                Err(e) => parser_err!(format!("Could not parse '{}' as number: {}", n, e)),
            },
            Token::SingleQuotedString(ref s) => Ok(Value::SingleQuotedString(s.to_string())),
            Token::NationalStringLiteral(ref s) => Ok(Value::NationalStringLiteral(s.to_string())),
            Token::HexStringLiteral(ref s) => Ok(Value::HexStringLiteral(s.to_string())),
            unexpected => self.expected("a value", unexpected),
        }
    }

    fn parse_column_def(&mut self) -> Result<ColumnDef, ParserError> {
        let name = self.parser.parse_identifier()?;
        let data_type = self.parser.parse_data_type()?;
        let collation = if self.parser.parse_keyword(Keyword::COLLATE) {
            Some(self.parser.parse_object_name()?)
        } else {
            None
        };
        let mut options = vec![];
        loop {
            if self.parser.parse_keyword(Keyword::CONSTRAINT) {
                let name = Some(self.parser.parse_identifier()?);
                if let Some(option) = self.parser.parse_optional_column_option()? {
                    options.push(ColumnOptionDef { name, option });
                } else {
                    return self.expected(
                        "constraint details after CONSTRAINT <name>",
                        self.parser.peek_token(),
                    );
                }
            } else if let Some(option) = self.parser.parse_optional_column_option()? {
                options.push(ColumnOptionDef { name: None, option });
            } else {
                break;
            };
        }
        Ok(ColumnDef {
            name,
            data_type,
            collation,
            options,
        })
    }

    fn parse_create(&mut self) -> Result<DfStatement, ParserError> {
        match self.parser.next_token() {
            Token::Word(w) => match w.keyword {
                Keyword::TABLE => self.parse_create_table(),
                Keyword::DATABASE => self.parse_create_database(),
                _ => self.expected("create statement", Token::Word(w)),
            },
            unexpected => self.expected("create statement", unexpected),
        }
    }

    fn parse_create_database(&mut self) -> Result<DfStatement, ParserError> {
        let if_not_exists =
            self.parser
                .parse_keywords(&[Keyword::IF, Keyword::NOT, Keyword::EXISTS]);
        let db_name = self.parser.parse_object_name()?;
        let engine = self.parse_database_engine()?;

        let create = DfCreateDatabase {
            if_not_exists,
            name: db_name,
            engine,
            options: vec![],
        };

        Ok(DfStatement::CreateDatabase(create))
    }

    fn parse_describe(&mut self) -> Result<DfStatement, ParserError> {
        let table_name = self.parser.parse_object_name()?;
        let desc = DfDescribeTable { name: table_name };
        Ok(DfStatement::DescribeTable(desc))
    }

    /// Drop database/table.
    fn parse_drop(&mut self) -> Result<DfStatement, ParserError> {
        match self.parser.next_token() {
            Token::Word(w) => match w.keyword {
                Keyword::DATABASE => self.parse_drop_database(),
                Keyword::TABLE => self.parse_drop_table(),
                _ => self.expected("drop statement", Token::Word(w)),
            },
            unexpected => self.expected("drop statement", unexpected),
        }
    }

    /// Drop database.
    fn parse_drop_database(&mut self) -> Result<DfStatement, ParserError> {
        let if_not_exists = self.parser.parse_keywords(&[Keyword::IF, Keyword::EXISTS]);
        let db_name = self.parser.parse_object_name()?;

        let drop = DfDropDatabase {
            if_exists: if_not_exists,
            name: db_name,
        };

        Ok(DfStatement::DropDatabase(drop))
    }

    /// Drop table.
    fn parse_drop_table(&mut self) -> Result<DfStatement, ParserError> {
        let if_not_exists = self.parser.parse_keywords(&[Keyword::IF, Keyword::EXISTS]);
        let table_name = self.parser.parse_object_name()?;

        let drop = DfDropTable {
            if_exists: if_not_exists,
            name: table_name,
        };

        Ok(DfStatement::DropTable(drop))
    }

    // Parse 'use database' db name.
    fn parse_use_database(&mut self) -> Result<DfStatement, ParserError> {
        if !self.consume_token("USE") {
            return self.expected("Must USE", self.parser.peek_token());
        }

        let name = self.parser.parse_object_name()?;
        Ok(DfStatement::UseDatabase(DfUseDatabase { name }))
    }

    // Parse 'KILL statement'.
    fn parse_kill<F>(&mut self, f: F) -> Result<DfStatement, ParserError>
    where F: Fn(DfKillStatement) -> DfStatement {
        Ok(f(DfKillStatement {
            object_id: self.parser.parse_identifier()?,
        }))
    }

    // Parse 'KILL statement'.
    fn parse_kill_query(&mut self) -> Result<DfStatement, ParserError> {
        match self.consume_token("KILL") {
            true if self.consume_token("QUERY") => self.parse_kill(DfStatement::KillQuery),
            true if self.consume_token("CONNECTION") => self.parse_kill(DfStatement::KillConn),
            true => self.parse_kill(DfStatement::KillConn),
            false => self.expected("Must KILL", self.parser.peek_token()),
        }
    }

    fn parse_database_engine(&mut self) -> Result<DatabaseEngineType, ParserError> {
        // TODO make ENGINE as a keyword
        if !self.consume_token("ENGINE") {
            return Ok(DatabaseEngineType::Remote);
        }

        self.parser.expect_token(&Token::Eq)?;

        match self.parser.next_token() {
            Token::Word(w) => match &*w.value {
                "Local" => Ok(DatabaseEngineType::Local),
                "Remote" => Ok(DatabaseEngineType::Remote),
                _ => self.expected("Engine must one of Local, Remote", Token::Word(w)),
            },
            unexpected => self.expected("Engine must one of Local, Remote", unexpected),
        }
    }

    fn parse_create_table(&mut self) -> Result<DfStatement, ParserError> {
        let if_not_exists =
            self.parser
                .parse_keywords(&[Keyword::IF, Keyword::NOT, Keyword::EXISTS]);
        let table_name = self.parser.parse_object_name()?;
        let (columns, _) = self.parse_columns()?;
        let engine = self.parse_table_engine()?;

        let mut table_properties = vec![];

        // parse table options: https://dev.mysql.com/doc/refman/8.0/en/create-table.html
        if self.consume_token("LOCATION") {
            self.parser.expect_token(&Token::Eq)?;
            let value = self.parse_value()?;
            table_properties.push(SqlOption {
                name: Ident::new("LOCATION"),
                value,
            })
        }

        let create = DfCreateTable {
            if_not_exists,
            name: table_name,
            columns,
            engine,
            options: table_properties,
        };

        Ok(DfStatement::CreateTable(create))
    }

    /// Parses the set of valid formats
    fn parse_table_engine(&mut self) -> Result<TableEngineType, ParserError> {
        // TODO make ENGINE as a keyword
        if !self.consume_token("ENGINE") {
            return Ok(TableEngineType::Null);
        }

        self.parser.expect_token(&Token::Eq)?;

        match self.parser.next_token() {
            Token::Word(w) => match &*w.value {
                "Parquet" => Ok(TableEngineType::Parquet),
                "JSONEachRow" => Ok(TableEngineType::JSONEachRow),
                "CSV" => Ok(TableEngineType::Csv),
                "Null" => Ok(TableEngineType::Null),
                "Memory" => Ok(TableEngineType::Memory),
                _ => self.expected(
                    "Engine must be one of Parquet, JSONEachRow, Null, Memory or CSV",
                    Token::Word(w),
                ),
            },
            unexpected => self.expected(
                "Engine must be one of Parquet, JSONEachRow, Null, Memory or CSV",
                unexpected,
            ),
        }
    }

    fn parse_show_create(&mut self) -> Result<DfStatement, ParserError> {
        match self.parser.next_token() {
            Token::Word(w) => match w.keyword {
                Keyword::TABLE => {
                    let table_name = self.parser.parse_object_name()?;

                    let show_create_table = DfShowCreateTable { name: table_name };
                    Ok(DfStatement::ShowCreateTable(show_create_table))
                }
                _ => self.expected("show create statement", Token::Word(w)),
            },
            unexpected => self.expected("show create statement", unexpected),
        }
    }

    fn parse_truncate(&mut self) -> Result<DfStatement, ParserError> {
        match self.parser.next_token() {
            Token::Word(w) => match w.keyword {
                Keyword::TABLE => {
                    let table_name = self.parser.parse_object_name()?;
                    let trunc = DfTruncateTable { name: table_name };
                    Ok(DfStatement::TruncateTable(trunc))
                }
                _ => self.expected("truncate statement", Token::Word(w)),
            },
            unexpected => self.expected("truncate statement", unexpected),
        }
    }

    fn consume_token(&mut self, expected: &str) -> bool {
        if self.parser.peek_token().to_string().to_uppercase() == *expected.to_uppercase() {
            self.parser.next_token();
            true
        } else {
            false
        }
    }
}
