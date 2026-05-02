//! Schema model — derived from `CREATE TABLE` statements found in the input.
//!
//! v0.3 scope: single-file schema only. The user-supplied SQL is scanned for
//! `CREATE TABLE` statements before any rule runs, and the resulting model
//! is passed to every rule via `LintCtx`. Rules that have schema-aware
//! refinements consult this model; rules that don't are unaffected.
//!
//! Lookups are case-insensitive (MariaDB column names are case-insensitive
//! by default; table names depend on `lower_case_table_names` but most
//! real-world setups treat them as case-insensitive too).
//!
//! Out of scope for v0.3:
//!   * `--schema <file>` external schema input — same-file only.
//!   * CREATE INDEX awareness (would help SQLT0503 function-on-column).
//!   * FOREIGN KEY / REFERENCES tracking.
//!   * CTE / VIEW / derived-table expansion (queries against CTEs are
//!     conservatively skipped).
//!   * Database / schema-qualified table names beyond `table.column`.

use std::collections::BTreeMap;

use sqlparser::ast::{ColumnDef, ColumnOption, DataType, ObjectName, ObjectNamePart, Statement};

use crate::ast::SqltStatement;

#[derive(Debug, Clone, Default)]
pub struct Schema {
    tables: BTreeMap<String, Table>,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub columns: BTreeMap<String, Column>,
}

#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
    /// `true` unless the column has a `NOT NULL` constraint. Primary-key
    /// columns are also treated as NOT NULL.
    pub nullable: bool,
}

impl Schema {
    /// Build a schema by walking every `CREATE TABLE` statement in the
    /// input. Statements are processed in source order; later definitions
    /// of the same table replace earlier ones (matching MariaDB behaviour
    /// when the second CREATE follows a DROP).
    pub fn from_statements(stmts: &[SqltStatement]) -> Self {
        let mut schema = Schema::default();
        for stmt in stmts {
            let SqltStatement::Std(boxed) = stmt else {
                continue;
            };
            let Statement::CreateTable(t) = &**boxed else {
                continue;
            };
            schema.add_table(&t.name, &t.columns);
        }
        schema
    }

    fn add_table(&mut self, name: &ObjectName, columns: &[ColumnDef]) {
        let table_name = object_name_last(name);
        let mut tbl = Table {
            name: table_name.clone(),
            columns: BTreeMap::new(),
        };
        for col in columns {
            let nullable = !col.options.iter().any(|o| {
                matches!(
                    o.option,
                    ColumnOption::NotNull
                        | ColumnOption::Unique {
                            is_primary: true,
                            ..
                        }
                )
            });
            tbl.columns.insert(
                col.name.value.to_ascii_lowercase(),
                Column {
                    name: col.name.value.clone(),
                    data_type: col.data_type.clone(),
                    nullable,
                },
            );
        }
        self.tables.insert(table_name.to_ascii_lowercase(), tbl);
    }

    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables.get(&name.to_ascii_lowercase())
    }

    pub fn column<'a>(&'a self, table: &str, column: &str) -> Option<&'a Column> {
        self.table(table)
            .and_then(|t| t.columns.get(&column.to_ascii_lowercase()))
    }

    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tables.len()
    }

    pub fn tables_iter(&self) -> impl Iterator<Item = &Table> {
        self.tables.values()
    }
}

fn object_name_last(name: &ObjectName) -> String {
    name.0
        .last()
        .and_then(|p| match p {
            ObjectNamePart::Identifier(i) => Some(i.value.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::DialectId;
    use crate::parse;

    fn schema_from(sql: &str) -> Schema {
        let stmts = parse::parse(sql, DialectId::MySql).expect("parse");
        Schema::from_statements(&stmts)
    }

    #[test]
    fn extracts_tables_and_columns() {
        let s = schema_from(
            "CREATE TABLE users (id INT NOT NULL, name VARCHAR(255), email VARCHAR(255) NOT NULL)",
        );
        assert_eq!(s.len(), 1);
        let t = s.table("users").expect("users present");
        assert!(t.columns.contains_key("id"));
        assert!(t.columns.contains_key("name"));
        assert!(s.column("users", "id").unwrap().nullable.eq(&false));
        assert!(s.column("users", "name").unwrap().nullable.eq(&true));
        assert!(s.column("users", "email").unwrap().nullable.eq(&false));
    }

    #[test]
    fn case_insensitive_lookup() {
        let s = schema_from("CREATE TABLE Users (Id INT NOT NULL)");
        assert!(s.table("USERS").is_some());
        assert!(s.column("users", "ID").is_some());
    }

    #[test]
    fn primary_key_is_not_null() {
        let s = schema_from("CREATE TABLE t (id INT PRIMARY KEY)");
        let c = s.column("t", "id").unwrap();
        assert!(
            !c.nullable,
            "primary key columns must be tracked as NOT NULL"
        );
    }

    #[test]
    fn empty_schema_when_no_create_table() {
        let s = schema_from("SELECT 1");
        assert!(s.is_empty());
    }
}
