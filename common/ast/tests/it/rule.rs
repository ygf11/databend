// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::io::Write;

use common_ast::parser::rule::error::pretty_print_error;
use common_ast::parser::rule::expr::expr;
use common_ast::parser::rule::statement::query;
use common_ast::parser::rule::statement::statement;
use common_ast::parser::token::*;
use goldenfile::Mint;
use nom::Parser;

macro_rules! test_parse {
    ($file:expr, $parser:expr, $source:expr $(,)*) => {
        let tokens = tokenise($source).unwrap();
        match $parser.parse(&(tokens)) {
            Ok((i, output)) if i[0].kind == TokenKind::EOI => {
                writeln!($file, "---------- Input ----------").unwrap();
                writeln!($file, "{}", $source).unwrap();
                writeln!($file, "---------- Output ---------").unwrap();
                writeln!($file, "{}", output).unwrap();
                writeln!($file, "---------- AST ------------").unwrap();
                writeln!($file, "{:#?}", output).unwrap();
                writeln!($file, "\n").unwrap();
            }
            Ok((i, output)) => {
                writeln!($file, "---------- Input ----------").unwrap();
                writeln!($file, "{}", $source).unwrap();
                writeln!($file, "---------- Output ---------").unwrap();
                writeln!($file, "{}", output).unwrap();
                writeln!($file, "---------- AST ------------").unwrap();
                writeln!($file, "{:#?}", output).unwrap();
                writeln!($file, "\n").unwrap();
                writeln!($file, "---------- REST -----------").unwrap();
                writeln!($file, "{:?}", i).unwrap();
            }
            Err(err) => {
                let report = pretty_print_error($source, err).trim_end().to_string();
                writeln!($file, "---------- Input ----------").unwrap();
                writeln!($file, "{}", $source).unwrap();
                writeln!($file, "---------- Output ---------").unwrap();
                writeln!($file, "{}", report).unwrap();
                writeln!($file, "\n").unwrap();
            }
        }
    };
}

#[test]
fn test_statement() {
    let mut mint = Mint::new("tests/it/testdata");
    let mut file = mint.new_goldenfile("statement.txt").unwrap();
    let cases = &[
        "show tables;",
        "show processlist;",
        "show create table a.b;",
        "explain analyze select a from b;",
        "describe a;",
        "create table if not exists a.b (c integer not null default 1, b varchar);",
        "create table if not exists a.b (c integer default 1 not null, b varchar);",
        "create table a.b like c.d;",
        "truncate table a;",
        r#"truncate table "a".b;"#,
        "drop table a;",
        r#"drop table if exists a."b";"#,
        r#"use "a";"#,
        "create database if not exists a;",
        "create table c(a DateTime null, b DateTime(3));",
    ];

    for case in cases {
        test_parse!(file, statement, case);
    }
}

#[test]
fn test_statement_error() {
    let mut mint = Mint::new("tests/it/testdata");
    let mut file = mint.new_goldenfile("statement-error.txt").unwrap();

    let cases = &[
        "create table a.b (c integer not null 1, b varchar(10));",
        "create table a (c varchar(10));",
        "create table a (c varch);",
        "drop table if a.b;",
        "truncate table a",
        "truncate table a.b.c.d",
        "truncate a",
        "drop a",
        "kill a",
    ];

    for case in cases {
        test_parse!(file, statement, case);
    }
}

#[test]
fn test_query() {
    let mut mint = Mint::new("tests/it/testdata");
    let mut file = mint.new_goldenfile("query.txt").unwrap();
    let cases = &[
        "select c_count, count(*) as custdist, sum(c_acctbal) as totacctbal
            from customer, orders ODS,
                (
                    select
                        c_custkey,
                        count(o_orderkey)
                    from
                        customer left outer join orders on
                            c_custkey = o_custkey
                            and o_comment not like '%:1%:2%'
                    group by
                        c_custkey
                ) as c_orders
            group by c_count
            order by custdist desc, c_count asc, totacctbal
            limit 10, totacctbal",
        "select * from customer inner join orders on a = b limit 1",
        "select * from customer natural full join orders",
        "select * from customer natural join orders left outer join detail using (id)",
    ];

    for case in cases {
        test_parse!(file, query, case);
    }
}

#[test]
fn test_query_error() {
    let mut mint = Mint::new("tests/it/testdata");
    let mut file = mint.new_goldenfile("query-error.txt").unwrap();
    let cases = &[
        "select * from customer join where a = b",
        "select * from join customer",
        "select * from customer natural inner join orders on a = b",
    ];

    for case in cases {
        test_parse!(file, query, case);
    }
}

#[test]
fn test_expr() {
    let mut mint = Mint::new("tests/it/testdata");
    let mut file = mint.new_goldenfile("expr.txt").unwrap();

    let cases = &[
        "a",
        "1 + a * c.d",
        "col1 not between 1 and 2",
        "sum(col1)",
        "rand()",
        "rand(distinct)",
        "CAST(col1 AS BIGINT UNSIGNED)",
        "G.E.B IS NOT NULL AND col1 not between col2 and (1 + col3) DIV sum(col4)",
        "sum(CASE WHEN n2.n_name = 'GERMANY' THEN ol_amount ELSE 0 END) / CASE WHEN sum(ol_amount) = 0 THEN 1 ELSE sum(ol_amount) END",
        "p_partkey = l_partkey
            AND p_brand = 'Brand#12'
            AND p_container IN ('SM CASE', 'SM BOX', 'SM PACK', 'SM PKG')
            AND l_quantity >= CAST (1 AS smallint) AND l_quantity <= CAST (1 + 10 AS smallint)
            AND p_size BETWEEN CAST (1 AS smallint) AND CAST (5 AS smallint)
            AND l_shipmode IN ('AIR', 'AIR REG')
            AND l_shipinstruct = 'DELIVER IN PERSON'",
    ];

    for case in cases {
        test_parse!(file, expr, case);
    }
}

#[test]
fn test_expr_error() {
    let mut mint = Mint::new("tests/it/testdata");
    let mut file = mint.new_goldenfile("expr-error.txt").unwrap();

    let cases = &[
        "5 * (a and ) 1",
        "a + +",
        "G.E.B IS NOT NULL AND\n\tcol1 NOT BETWEEN col2 AND\n\t\tAND 1 + col3 DIV sum(col4)",
    ];

    for case in cases {
        test_parse!(file, expr, case);
    }
}
