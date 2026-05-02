//! Schema model — derived from CREATE/ALTER/DROP TABLE statements.
//!
//! Two ways to build a `Schema`:
//!   * `Schema::from_statements(stmts)` — single-input model used by the
//!     in-process lint when the user does not pass `--schema`. Walks the
//!     parsed statements once, applies CREATE TABLE only (back-compat for
//!     v0.3 lint behaviour where the schema is "whatever CREATE TABLE you
//!     find in the input").
//!   * `Schema::default()` + repeated `apply_statement` calls — used by the
//!     `--schema <file>` loader and by `sqlt build-schema`. Replays the full
//!     DDL surface (CREATE/ALTER/DROP TABLE, CREATE INDEX, FK constraints,
//!     CREATE DATABASE, USE) so that what the linter sees is the *current*
//!     state of the schema, not the initial CREATE.
//!
//! Per-database namespacing: every `Table` lives in a `Database` keyed by
//! its case-insensitive name. The `current_db` cursor tracks `USE <db>` so
//! that subsequent unqualified `CREATE TABLE foo` statements land in the
//! right namespace. The cursor persists across calls to `apply_statement`,
//! mirroring how `mysql -e "source a.sql; source b.sql"` works in real life.
//!
//! Lookups are case-insensitive (MariaDB column names are case-insensitive
//! by default; table names depend on `lower_case_table_names` but most
//! real-world setups treat them as case-insensitive too).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sqlparser::ast::Spanned;
use sqlparser::ast::{
    AlterTableOperation, ColumnDef, ColumnOption, ColumnOptionDef, CreateIndex,
    CreateTableLikeKind, DataType, IndexColumn, ObjectName, ObjectNamePart, ObjectType,
    RenameTableNameKind, Statement, TableConstraint, Use,
};

use crate::ast::SqltStatement;

/// Sentinel for the implicit/unnamed database — used when a CREATE TABLE
/// has no preceding `USE` and no DB-qualified name.
pub const DEFAULT_DB: &str = "";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Schema {
    /// Pinned at serialize time so `--schema schema.json` can warn on
    /// version skew. Empty for in-memory-only schemas.
    #[serde(default)]
    pub sqlt_version: String,
    /// Database namespaces, keyed by lowercased name. The empty string is
    /// the implicit/unnamed database that holds tables defined without a
    /// preceding `USE` or 2-part `db.table` name.
    pub databases: BTreeMap<String, Database>,
    /// Most recent `USE <db>` (lowercased). Persists across files in the
    /// CLI input order so a `USE shop_db` at the bottom of file `a.sql`
    /// affects the unqualified CREATE TABLEs at the top of file `b.sql`.
    pub current_db: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Database {
    pub name: String,
    pub tables: BTreeMap<String, Table>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub database: String,
    pub columns: BTreeMap<String, Column>,
    #[serde(default)]
    pub indexes: Vec<Index>,
    #[serde(default)]
    pub primary_key: Vec<String>,
    #[serde(default)]
    pub foreign_keys: Vec<ForeignKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
    /// `true` unless the column has a `NOT NULL` constraint or appears in
    /// a `PRIMARY KEY`.
    pub nullable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Index {
    pub name: Option<String>,
    /// Original-case column names (or rendered SQL for functional indexes).
    pub columns: Vec<String>,
    pub unique: bool,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub fulltext: bool,
    #[serde(default)]
    pub spatial: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKey {
    pub name: Option<String>,
    pub columns: Vec<String>,
    pub ref_db: String,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
}

/// Things `apply_statement` couldn't apply or noticed something off. The
/// caller (CLI or build-schema) renders these as `note:` lines on stderr.
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaSkip {
    /// Statement we don't replay (INSERT, GRANT, raw fallback, …).
    Unknown {
        kind: String,
        file: PathBuf,
        line: u64,
    },
    AlterMissingTable {
        table: String,
        file: PathBuf,
        line: u64,
    },
    DropMissingTable {
        table: String,
        file: PathBuf,
        line: u64,
    },
    /// CREATE TABLE for a table that already exists, with no `IF NOT EXISTS`
    /// and no preceding DROP. Per the user-locked-in "warn-only" policy we
    /// note it and overwrite anyway.
    DuplicateTable {
        table: String,
        file: PathBuf,
        line: u64,
    },
}

impl SchemaSkip {
    /// Render as a single human-readable line for `note: <text>` stderr
    /// output. The `note:` prefix is added by the caller.
    pub fn render(&self) -> String {
        match self {
            SchemaSkip::Unknown { kind, file, line } => {
                format!("skipping {kind} at {}:{line}", file.display())
            }
            SchemaSkip::AlterMissingTable { table, file, line } => format!(
                "ALTER TABLE on missing table `{table}` at {}:{line} (no IF EXISTS)",
                file.display()
            ),
            SchemaSkip::DropMissingTable { table, file, line } => format!(
                "DROP TABLE on missing table `{table}` at {}:{line} (no IF EXISTS)",
                file.display()
            ),
            SchemaSkip::DuplicateTable { table, file, line } => format!(
                "duplicate CREATE TABLE for `{table}` at {}:{line} — overwriting",
                file.display()
            ),
        }
    }
}

impl Schema {
    /// Convenience: build a schema by replaying CREATE TABLE statements
    /// from the parsed input. Used by the in-process lint pass when no
    /// `--schema` files were supplied. ALTER/DROP/CREATE INDEX from the
    /// lint input are intentionally ignored here — the assumption is that
    /// the input is queries plus maybe a few helper CREATE TABLEs, not a
    /// migration script.
    pub fn from_statements(stmts: &[SqltStatement]) -> Self {
        let mut schema = Schema::default();
        let synthetic = Path::new("<input>");
        let mut skips = Vec::new();
        for stmt in stmts {
            let SqltStatement::Std(boxed) = stmt else {
                continue;
            };
            if matches!(&**boxed, Statement::CreateTable(_)) {
                schema.apply_statement(stmt, synthetic, &mut skips);
            }
        }
        schema
    }

    /// Apply one parsed statement to this schema. Mutates the schema for
    /// schema-affecting DDL (CREATE/ALTER/DROP TABLE, CREATE INDEX, USE,
    /// CREATE DATABASE, …). Pushes a [`SchemaSkip`] for statements we
    /// don't recognize as schema-affecting OR for ALTER/DROP that target
    /// a non-existent table without an `IF EXISTS` guard.
    pub fn apply_statement(
        &mut self,
        stmt: &SqltStatement,
        file: &Path,
        skips: &mut Vec<SchemaSkip>,
    ) {
        let line = stmt_line(stmt);
        let boxed = match stmt {
            SqltStatement::Std(b) => b,
            SqltStatement::Raw(r) => {
                skips.push(SchemaSkip::Unknown {
                    kind: format!("raw:{}", r.reason),
                    file: file.to_path_buf(),
                    line,
                });
                return;
            }
        };
        match &**boxed {
            // ── DB cursor / namespace ────────────────────────────────────
            Statement::CreateDatabase {
                db_name,
                if_not_exists,
                ..
            } => {
                let name = object_name_last(db_name);
                let key = name.to_ascii_lowercase();
                if !*if_not_exists {
                    self.databases
                        .entry(key.clone())
                        .or_insert_with(|| Database {
                            name: name.clone(),
                            tables: BTreeMap::new(),
                        });
                } else {
                    self.databases.entry(key).or_insert_with(|| Database {
                        name,
                        tables: BTreeMap::new(),
                    });
                }
            }
            Statement::Use(Use::Database(name) | Use::Schema(name) | Use::Object(name)) => {
                let raw = object_name_last(name);
                let key = raw.to_ascii_lowercase();
                self.databases
                    .entry(key.clone())
                    .or_insert_with(|| Database {
                        name: raw,
                        tables: BTreeMap::new(),
                    });
                self.current_db = Some(key);
            }
            Statement::Use(_) => {
                // Catalog/Warehouse/Role/SecondaryRoles/Default — not schema-affecting.
            }

            // ── CREATE/ALTER/DROP TABLE ──────────────────────────────────
            Statement::CreateTable(t) => {
                self.apply_create_table(t, file, line, skips);
            }
            Statement::AlterTable {
                name,
                if_exists,
                operations,
                ..
            } => {
                self.apply_alter_table(name, *if_exists, operations, file, line, skips);
            }
            Statement::Drop {
                object_type,
                if_exists,
                names,
                table,
                ..
            } => match object_type {
                ObjectType::Table => {
                    for n in names {
                        self.apply_drop_table(n, *if_exists, file, line, skips);
                    }
                }
                ObjectType::Index => {
                    if let Some(t_name) = table {
                        let (db, t) = self.resolve_table_name(t_name);
                        if let Some(tbl) = self.table_qualified_mut(&db, &t) {
                            for index_name in names {
                                let target = object_name_last(index_name).to_ascii_lowercase();
                                tbl.indexes.retain(|ix| {
                                    ix.name
                                        .as_deref()
                                        .map(|n| n.to_ascii_lowercase() != target)
                                        .unwrap_or(true)
                                });
                            }
                        }
                    }
                }
                ObjectType::Database | ObjectType::Schema => {
                    for n in names {
                        let key = object_name_last(n).to_ascii_lowercase();
                        self.databases.remove(&key);
                        if self.current_db.as_deref() == Some(&key) {
                            self.current_db = None;
                        }
                    }
                }
                _ => {
                    skips.push(SchemaSkip::Unknown {
                        kind: format!("Drop({object_type:?})"),
                        file: file.to_path_buf(),
                        line,
                    });
                }
            },

            // ── CREATE INDEX ─────────────────────────────────────────────
            Statement::CreateIndex(ci) => {
                self.apply_create_index(ci, file, line, skips);
            }

            // ── Anything else: stderr note ───────────────────────────────
            other => {
                skips.push(SchemaSkip::Unknown {
                    kind: short_label(other),
                    file: file.to_path_buf(),
                    line,
                });
            }
        }
    }

    fn apply_create_table(
        &mut self,
        t: &sqlparser::ast::CreateTable,
        file: &Path,
        line: u64,
        skips: &mut Vec<SchemaSkip>,
    ) {
        let (db, table_name) = self.resolve_table_name(&t.name);
        let db_key = db.to_ascii_lowercase();
        let table_key = table_name.to_ascii_lowercase();

        let already_exists = self
            .databases
            .get(&db_key)
            .is_some_and(|d| d.tables.contains_key(&table_key));

        if already_exists {
            if t.if_not_exists {
                return;
            }
            if t.or_replace {
                // Drop first, then proceed to insert.
                if let Some(d) = self.databases.get_mut(&db_key) {
                    d.tables.remove(&table_key);
                }
            } else {
                skips.push(SchemaSkip::DuplicateTable {
                    table: format!("{db}.{table_name}"),
                    file: file.to_path_buf(),
                    line,
                });
                // Fall through and overwrite, matching MariaDB end-state.
            }
        }

        let mut tbl = Table {
            name: table_name.clone(),
            database: db.clone(),
            columns: BTreeMap::new(),
            indexes: Vec::new(),
            primary_key: Vec::new(),
            foreign_keys: Vec::new(),
        };
        for col in &t.columns {
            insert_column_def(&mut tbl, col);
        }
        for c in &t.constraints {
            apply_constraint(&mut tbl, c, self.current_db.as_deref());
        }

        // CREATE TABLE … LIKE other: copy columns from the source if known.
        if t.columns.is_empty()
            && let Some(like) = &t.like
        {
            let src_name = match like {
                CreateTableLikeKind::Parenthesized(c) | CreateTableLikeKind::Plain(c) => &c.name,
            };
            if let Some(src) = self.resolve_and_table(src_name) {
                tbl.columns = src.columns.clone();
                tbl.indexes = src.indexes.clone();
                tbl.primary_key = src.primary_key.clone();
                tbl.foreign_keys = src.foreign_keys.clone();
            }
        }

        let database = self.databases.entry(db_key).or_insert_with(|| Database {
            name: db.clone(),
            tables: BTreeMap::new(),
        });
        database.tables.insert(table_key, tbl);
    }

    fn apply_alter_table(
        &mut self,
        name: &ObjectName,
        if_exists: bool,
        operations: &[AlterTableOperation],
        file: &Path,
        line: u64,
        skips: &mut Vec<SchemaSkip>,
    ) {
        let (db, table_name) = self.resolve_table_name(name);
        let exists = self.table_qualified(&db, &table_name).is_some();
        if !exists {
            if !if_exists {
                skips.push(SchemaSkip::AlterMissingTable {
                    table: format!("{db}.{table_name}"),
                    file: file.to_path_buf(),
                    line,
                });
            }
            return;
        }
        let db_lower = db.to_ascii_lowercase();
        let cursor = self.current_db.clone();
        // Apply renames last so other ops use the original name. Common
        // pattern: a single ALTER never combines rename with column ops in
        // practice; but applying renames at end still works correctly for
        // multi-op ALTERs.
        let mut deferred_rename: Option<&RenameTableNameKind> = None;
        for op in operations {
            match op {
                AlterTableOperation::AddColumn {
                    if_not_exists,
                    column_def,
                    ..
                } => {
                    let tbl = self
                        .table_qualified_mut(&db_lower, &table_name)
                        .expect("table existed at start of alter");
                    let key = column_def.name.value.to_ascii_lowercase();
                    if *if_not_exists && tbl.columns.contains_key(&key) {
                        continue;
                    }
                    insert_column_def(tbl, column_def);
                }
                AlterTableOperation::DropColumn {
                    column_names,
                    if_exists,
                    ..
                } => {
                    let tbl = self
                        .table_qualified_mut(&db_lower, &table_name)
                        .expect("table");
                    for col in column_names {
                        let key = col.value.to_ascii_lowercase();
                        if !tbl.columns.contains_key(&key) && !*if_exists {
                            continue;
                        }
                        tbl.columns.remove(&key);
                        tbl.primary_key
                            .retain(|n| !n.eq_ignore_ascii_case(&col.value));
                        for ix in tbl.indexes.iter_mut() {
                            ix.columns.retain(|c| !c.eq_ignore_ascii_case(&col.value));
                        }
                        tbl.indexes.retain(|ix| !ix.columns.is_empty());
                    }
                }
                AlterTableOperation::ModifyColumn {
                    col_name,
                    data_type,
                    options,
                    ..
                } => {
                    let tbl = self
                        .table_qualified_mut(&db_lower, &table_name)
                        .expect("table");
                    let key = col_name.value.to_ascii_lowercase();
                    let nullable = !options.iter().any(|o| {
                        matches!(
                            o,
                            ColumnOption::NotNull
                                | ColumnOption::Unique {
                                    is_primary: true,
                                    ..
                                }
                        )
                    });
                    tbl.columns.insert(
                        key,
                        Column {
                            name: col_name.value.clone(),
                            data_type: data_type.clone(),
                            nullable,
                        },
                    );
                }
                AlterTableOperation::ChangeColumn {
                    old_name,
                    new_name,
                    data_type,
                    options,
                    ..
                } => {
                    let tbl = self
                        .table_qualified_mut(&db_lower, &table_name)
                        .expect("table");
                    let old_key = old_name.value.to_ascii_lowercase();
                    let new_key = new_name.value.to_ascii_lowercase();
                    tbl.columns.remove(&old_key);
                    let nullable = !options.iter().any(|o| {
                        matches!(
                            o,
                            ColumnOption::NotNull
                                | ColumnOption::Unique {
                                    is_primary: true,
                                    ..
                                }
                        )
                    });
                    tbl.columns.insert(
                        new_key,
                        Column {
                            name: new_name.value.clone(),
                            data_type: data_type.clone(),
                            nullable,
                        },
                    );
                    rename_in_pk_and_indexes(tbl, &old_name.value, &new_name.value);
                }
                AlterTableOperation::RenameColumn {
                    old_column_name,
                    new_column_name,
                } => {
                    let tbl = self
                        .table_qualified_mut(&db_lower, &table_name)
                        .expect("table");
                    let old_key = old_column_name.value.to_ascii_lowercase();
                    if let Some(mut col) = tbl.columns.remove(&old_key) {
                        col.name = new_column_name.value.clone();
                        tbl.columns
                            .insert(new_column_name.value.to_ascii_lowercase(), col);
                        rename_in_pk_and_indexes(
                            tbl,
                            &old_column_name.value,
                            &new_column_name.value,
                        );
                    }
                }
                AlterTableOperation::AddConstraint { constraint, .. } => {
                    let tbl = self
                        .table_qualified_mut(&db_lower, &table_name)
                        .expect("table");
                    apply_constraint(tbl, constraint, cursor.as_deref());
                }
                AlterTableOperation::DropConstraint { name, .. } => {
                    let tbl = self
                        .table_qualified_mut(&db_lower, &table_name)
                        .expect("table");
                    let target = name.value.to_ascii_lowercase();
                    tbl.foreign_keys.retain(|fk| {
                        fk.name
                            .as_deref()
                            .map(|n| n.to_ascii_lowercase() != target)
                            .unwrap_or(true)
                    });
                    tbl.indexes.retain(|ix| {
                        ix.name
                            .as_deref()
                            .map(|n| n.to_ascii_lowercase() != target)
                            .unwrap_or(true)
                    });
                }
                AlterTableOperation::DropPrimaryKey { .. } => {
                    let tbl = self
                        .table_qualified_mut(&db_lower, &table_name)
                        .expect("table");
                    tbl.primary_key.clear();
                    tbl.indexes.retain(|ix| !ix.primary);
                }
                AlterTableOperation::DropForeignKey { name, .. } => {
                    let tbl = self
                        .table_qualified_mut(&db_lower, &table_name)
                        .expect("table");
                    let target = name.value.to_ascii_lowercase();
                    tbl.foreign_keys.retain(|fk| {
                        fk.name
                            .as_deref()
                            .map(|n| n.to_ascii_lowercase() != target)
                            .unwrap_or(true)
                    });
                }
                AlterTableOperation::DropIndex { name } => {
                    let tbl = self
                        .table_qualified_mut(&db_lower, &table_name)
                        .expect("table");
                    let target = name.value.to_ascii_lowercase();
                    tbl.indexes.retain(|ix| {
                        ix.name
                            .as_deref()
                            .map(|n| n.to_ascii_lowercase() != target)
                            .unwrap_or(true)
                    });
                }
                AlterTableOperation::RenameTable { table_name } => {
                    deferred_rename = Some(table_name);
                }
                _ => {
                    // Postgres replica-identity, ClickHouse projection ops,
                    // etc. — known irrelevant; don't note.
                }
            }
        }
        if let Some(rk) = deferred_rename {
            let target = match rk {
                RenameTableNameKind::To(n) | RenameTableNameKind::As(n) => n,
            };
            self.rename_table(&db_lower, &table_name, target);
        }
    }

    fn apply_drop_table(
        &mut self,
        name: &ObjectName,
        if_exists: bool,
        file: &Path,
        line: u64,
        skips: &mut Vec<SchemaSkip>,
    ) {
        let (db, table_name) = self.resolve_table_name(name);
        let db_key = db.to_ascii_lowercase();
        let table_key = table_name.to_ascii_lowercase();
        let removed = self
            .databases
            .get_mut(&db_key)
            .and_then(|d| d.tables.remove(&table_key))
            .is_some();
        if !removed && !if_exists {
            skips.push(SchemaSkip::DropMissingTable {
                table: format!("{db}.{table_name}"),
                file: file.to_path_buf(),
                line,
            });
        }
    }

    fn apply_create_index(
        &mut self,
        ci: &CreateIndex,
        file: &Path,
        line: u64,
        skips: &mut Vec<SchemaSkip>,
    ) {
        let (db, table_name) = self.resolve_table_name(&ci.table_name);
        let Some(tbl) = self.table_qualified_mut(&db.to_ascii_lowercase(), &table_name) else {
            skips.push(SchemaSkip::AlterMissingTable {
                table: format!("{db}.{table_name}"),
                file: file.to_path_buf(),
                line,
            });
            return;
        };
        let name = ci.name.as_ref().map(object_name_last);
        if ci.if_not_exists
            && let Some(n) = &name
            && tbl
                .indexes
                .iter()
                .any(|ix| ix.name.as_deref() == Some(n.as_str()))
        {
            return;
        }
        tbl.indexes.push(Index {
            name,
            columns: ci.columns.iter().map(render_index_column).collect(),
            unique: ci.unique,
            primary: false,
            fulltext: false,
            spatial: false,
        });
    }

    fn rename_table(&mut self, src_db: &str, src_table: &str, target: &ObjectName) {
        let (dst_db, dst_table) = self.resolve_table_name(target);
        let dst_db_key = dst_db.to_ascii_lowercase();
        let dst_table_key = dst_table.to_ascii_lowercase();
        let src_table_key = src_table.to_ascii_lowercase();
        let Some(src_db_entry) = self.databases.get_mut(src_db) else {
            return;
        };
        let Some(mut tbl) = src_db_entry.tables.remove(&src_table_key) else {
            return;
        };
        tbl.name = dst_table.clone();
        tbl.database = dst_db.clone();
        let dst_db_entry = self
            .databases
            .entry(dst_db_key)
            .or_insert_with(|| Database {
                name: dst_db,
                tables: BTreeMap::new(),
            });
        dst_db_entry.tables.insert(dst_table_key, tbl);
    }

    fn resolve_and_table(&self, name: &ObjectName) -> Option<&Table> {
        let (db, t) = self.resolve_table_name(name);
        self.table_qualified(&db, &t)
    }

    /// Resolve a possibly-qualified ObjectName into `(db, table)`, honoring
    /// the `current_db` cursor for unqualified names. 3-part names
    /// (`catalog.db.table`) drop the catalog.
    pub fn resolve_table_name(&self, name: &ObjectName) -> (String, String) {
        let parts: Vec<&str> = name
            .0
            .iter()
            .filter_map(|p| match p {
                ObjectNamePart::Identifier(i) => Some(i.value.as_str()),
                _ => None,
            })
            .collect();
        match parts.as_slice() {
            [t] => (
                self.current_db.clone().unwrap_or_else(|| DEFAULT_DB.into()),
                (*t).to_string(),
            ),
            [db, t] => ((*db).to_string(), (*t).to_string()),
            [_catalog, db, t] => ((*db).to_string(), (*t).to_string()),
            _ => (
                self.current_db.clone().unwrap_or_else(|| DEFAULT_DB.into()),
                parts.last().copied().unwrap_or("").to_string(),
            ),
        }
    }

    pub fn column_qualified(&self, db: &str, table: &str, column: &str) -> Option<&Column> {
        self.databases
            .get(&db.to_ascii_lowercase())
            .and_then(|d| d.tables.get(&table.to_ascii_lowercase()))
            .and_then(|t| t.columns.get(&column.to_ascii_lowercase()))
    }

    pub fn table_qualified(&self, db: &str, table: &str) -> Option<&Table> {
        self.databases
            .get(&db.to_ascii_lowercase())
            .and_then(|d| d.tables.get(&table.to_ascii_lowercase()))
    }

    fn table_qualified_mut(&mut self, db_lower: &str, table: &str) -> Option<&mut Table> {
        self.databases
            .get_mut(db_lower)
            .and_then(|d| d.tables.get_mut(&table.to_ascii_lowercase()))
    }

    pub fn table(&self, name: &str) -> Option<&Table> {
        let key = name.to_ascii_lowercase();
        if let Some(db) = self.current_db.as_deref()
            && let Some(t) = self
                .databases
                .get(&db.to_ascii_lowercase())
                .and_then(|d| d.tables.get(&key))
        {
            return Some(t);
        }
        if let Some(t) = self
            .databases
            .get(DEFAULT_DB)
            .and_then(|d| d.tables.get(&key))
        {
            return Some(t);
        }
        self.tables_iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
    }

    pub fn column<'a>(&'a self, table: &str, column: &str) -> Option<&'a Column> {
        self.table(table)
            .and_then(|t| t.columns.get(&column.to_ascii_lowercase()))
    }

    pub fn is_empty(&self) -> bool {
        self.databases.values().all(|d| d.tables.is_empty())
    }

    pub fn len(&self) -> usize {
        self.databases.values().map(|d| d.tables.len()).sum()
    }

    pub fn tables_iter(&self) -> impl Iterator<Item = &Table> {
        self.databases.values().flat_map(|d| d.tables.values())
    }

    /// Merge another schema (typically from a `--schema schema.json` load)
    /// into this one. Last write wins; the loaded `current_db` overrides.
    pub fn merge(&mut self, other: &Schema) {
        for (key, db) in &other.databases {
            let entry = self
                .databases
                .entry(key.clone())
                .or_insert_with(|| Database {
                    name: db.name.clone(),
                    tables: BTreeMap::new(),
                });
            for (tk, t) in &db.tables {
                entry.tables.insert(tk.clone(), t.clone());
            }
        }
        if other.current_db.is_some() {
            self.current_db = other.current_db.clone();
        }
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn insert_column_def(tbl: &mut Table, col: &ColumnDef) {
    let nullable = !col.options.iter().any(is_not_null_def);
    if col.options.iter().any(|o| {
        matches!(
            o.option,
            ColumnOption::Unique {
                is_primary: true,
                ..
            }
        )
    }) {
        tbl.primary_key.push(col.name.value.clone());
    }
    tbl.columns.insert(
        col.name.value.to_ascii_lowercase(),
        Column {
            name: col.name.value.clone(),
            data_type: col.data_type.clone(),
            nullable,
        },
    );
}

fn is_not_null_def(o: &ColumnOptionDef) -> bool {
    matches!(
        o.option,
        ColumnOption::NotNull
            | ColumnOption::Unique {
                is_primary: true,
                ..
            }
    )
}

fn apply_constraint(tbl: &mut Table, c: &TableConstraint, current_db: Option<&str>) {
    match c {
        TableConstraint::PrimaryKey { columns, .. } => {
            let cols: Vec<String> = columns.iter().map(render_index_column).collect();
            // Mark each column NOT NULL if it appears in the PK.
            for cname in &cols {
                if let Some(col) = tbl.columns.get_mut(&cname.to_ascii_lowercase()) {
                    col.nullable = false;
                }
                if !tbl
                    .primary_key
                    .iter()
                    .any(|n| n.eq_ignore_ascii_case(cname))
                {
                    tbl.primary_key.push(cname.clone());
                }
            }
            tbl.indexes.push(Index {
                name: None,
                columns: cols,
                unique: true,
                primary: true,
                fulltext: false,
                spatial: false,
            });
        }
        TableConstraint::Unique {
            name,
            index_name,
            columns,
            ..
        } => {
            tbl.indexes.push(Index {
                name: name
                    .as_ref()
                    .or(index_name.as_ref())
                    .map(|i| i.value.clone()),
                columns: columns.iter().map(render_index_column).collect(),
                unique: true,
                primary: false,
                fulltext: false,
                spatial: false,
            });
        }
        TableConstraint::Index { name, columns, .. } => {
            tbl.indexes.push(Index {
                name: name.as_ref().map(|i| i.value.clone()),
                columns: columns.iter().map(render_index_column).collect(),
                unique: false,
                primary: false,
                fulltext: false,
                spatial: false,
            });
        }
        TableConstraint::FulltextOrSpatial {
            fulltext,
            opt_index_name,
            columns,
            ..
        } => {
            tbl.indexes.push(Index {
                name: opt_index_name.as_ref().map(|i| i.value.clone()),
                columns: columns.iter().map(render_index_column).collect(),
                unique: false,
                primary: false,
                fulltext: *fulltext,
                spatial: !*fulltext,
            });
        }
        TableConstraint::ForeignKey {
            name,
            columns,
            foreign_table,
            referred_columns,
            ..
        } => {
            let (ref_db, ref_table) = resolve_object_name(foreign_table, current_db);
            tbl.foreign_keys.push(ForeignKey {
                name: name.as_ref().map(|i| i.value.clone()),
                columns: columns.iter().map(|c| c.value.clone()).collect(),
                ref_db,
                ref_table,
                ref_columns: referred_columns.iter().map(|c| c.value.clone()).collect(),
            });
        }
        TableConstraint::Check { .. } => {
            // Not modeled; intentionally not skip-noted (it's expected).
        }
    }
}

fn rename_in_pk_and_indexes(tbl: &mut Table, old: &str, new: &str) {
    for c in tbl.primary_key.iter_mut() {
        if c.eq_ignore_ascii_case(old) {
            *c = new.to_string();
        }
    }
    for ix in tbl.indexes.iter_mut() {
        for c in ix.columns.iter_mut() {
            if c.eq_ignore_ascii_case(old) {
                *c = new.to_string();
            }
        }
    }
}

fn render_index_column(ic: &IndexColumn) -> String {
    // For plain identifiers this yields the column name. For functional
    // indexes (e.g. `(LOWER(email))`) it yields the rendered SQL expression
    // — useful for matching SQLT0503-style "indexed by `LOWER(email)`".
    ic.column.expr.to_string()
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

/// Like `Schema::resolve_table_name` but works without a live `&Schema`
/// (used by `apply_constraint` for foreign-key target resolution).
fn resolve_object_name(name: &ObjectName, current_db: Option<&str>) -> (String, String) {
    let parts: Vec<&str> = name
        .0
        .iter()
        .filter_map(|p| match p {
            ObjectNamePart::Identifier(i) => Some(i.value.as_str()),
            _ => None,
        })
        .collect();
    match parts.as_slice() {
        [t] => (
            current_db.unwrap_or(DEFAULT_DB).to_string(),
            (*t).to_string(),
        ),
        [db, t] => ((*db).to_string(), (*t).to_string()),
        [_catalog, db, t] => ((*db).to_string(), (*t).to_string()),
        _ => (
            current_db.unwrap_or(DEFAULT_DB).to_string(),
            parts.last().copied().unwrap_or("").to_string(),
        ),
    }
}

/// Short, human-readable label for a Statement variant — for skip notes.
fn short_label(stmt: &Statement) -> String {
    match stmt {
        Statement::Insert(_) => "Insert".into(),
        Statement::Update { .. } => "Update".into(),
        Statement::Delete(_) => "Delete".into(),
        Statement::Set(_) => "Set".into(),
        Statement::Query(_) => "Query".into(),
        Statement::CreateView { .. } => "CreateView".into(),
        Statement::CreateTrigger(_) => "CreateTrigger".into(),
        Statement::CreateFunction(_) => "CreateFunction".into(),
        Statement::CreateProcedure { .. } => "CreateProcedure".into(),
        Statement::Comment { .. } => "Comment".into(),
        Statement::Commit { .. }
        | Statement::Rollback { .. }
        | Statement::StartTransaction { .. } => "Transaction".into(),
        Statement::CreateSequence { .. } => "CreateSequence".into(),
        Statement::ShowVariable { .. }
        | Statement::ShowVariables { .. }
        | Statement::ShowTables { .. }
        | Statement::ShowColumns { .. } => "Show".into(),
        Statement::Truncate { .. } => "Truncate".into(),
        Statement::AlterRole { .. } => "AlterRole".into(),
        Statement::AlterIndex { .. } => "AlterIndex".into(),
        Statement::AlterView { .. } => "AlterView".into(),
        _ => "OtherDDL".into(),
    }
}

fn stmt_line(stmt: &SqltStatement) -> u64 {
    match stmt {
        SqltStatement::Std(boxed) => {
            let s = boxed.span();
            if s.start.line == 0 { 1 } else { s.start.line }
        }
        SqltStatement::Raw(r) => r.start_line.unwrap_or(1),
    }
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

    /// Helper for tests that need full DDL replay (not just CREATE TABLE).
    fn replay(sql: &str) -> (Schema, Vec<SchemaSkip>) {
        let stmts = parse::parse(sql, DialectId::MySql).expect("parse");
        let mut s = Schema::default();
        let mut skips = Vec::new();
        let p = Path::new("<test>");
        for stmt in &stmts {
            s.apply_statement(stmt, p, &mut skips);
        }
        (s, skips)
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
        assert!(!c.nullable);
    }

    #[test]
    fn primary_key_recorded_on_table() {
        let s = schema_from("CREATE TABLE t (id INT PRIMARY KEY, name VARCHAR(50))");
        let t = s.table("t").unwrap();
        assert_eq!(t.primary_key, vec!["id".to_string()]);
    }

    #[test]
    fn empty_schema_when_no_create_table() {
        let s = schema_from("SELECT 1");
        assert!(s.is_empty());
    }

    #[test]
    fn schema_implements_serde_round_trip() {
        let s = schema_from("CREATE TABLE t (id INT NOT NULL, name VARCHAR(50))");
        let json = serde_json::to_string(&s).expect("serialize");
        let back: Schema = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.len(), 1);
        assert!(!back.column("t", "id").unwrap().nullable);
    }

    #[test]
    fn alter_add_column() {
        let (s, skips) = replay(
            "CREATE TABLE t (id INT); \
             ALTER TABLE t ADD COLUMN email VARCHAR(255) NOT NULL",
        );
        assert!(skips.is_empty());
        assert!(s.column("t", "email").is_some());
        assert!(!s.column("t", "email").unwrap().nullable);
    }

    #[test]
    fn alter_drop_column() {
        let (s, _) = replay("CREATE TABLE t (id INT, b INT); ALTER TABLE t DROP COLUMN b");
        assert!(s.column("t", "id").is_some());
        assert!(s.column("t", "b").is_none());
    }

    #[test]
    fn alter_modify_column_changes_type_and_nullability() {
        let (s, _) = replay(
            "CREATE TABLE t (id INT NOT NULL); \
             ALTER TABLE t MODIFY COLUMN id BIGINT NULL",
        );
        let c = s.column("t", "id").unwrap();
        assert!(c.nullable);
    }

    #[test]
    fn alter_change_column_renames_and_replaces() {
        let (s, _) = replay(
            "CREATE TABLE t (old_name VARCHAR(10)); \
             ALTER TABLE t CHANGE COLUMN old_name new_name VARCHAR(20) NOT NULL",
        );
        assert!(s.column("t", "old_name").is_none());
        let c = s.column("t", "new_name").unwrap();
        assert!(!c.nullable);
    }

    #[test]
    fn alter_rename_column_keeps_type() {
        let (s, _) = replay(
            "CREATE TABLE t (a INT NOT NULL); \
             ALTER TABLE t RENAME COLUMN a TO b",
        );
        assert!(s.column("t", "a").is_none());
        assert!(!s.column("t", "b").unwrap().nullable);
    }

    #[test]
    fn alter_rename_table_within_db() {
        let (s, _) = replay(
            "CREATE TABLE t (id INT); \
             ALTER TABLE t RENAME TO u",
        );
        assert!(s.table("t").is_none());
        assert!(s.table("u").is_some());
    }

    #[test]
    fn drop_table_removes_it() {
        let (s, _) = replay("CREATE TABLE t (id INT); DROP TABLE t");
        assert!(s.table("t").is_none());
    }

    #[test]
    fn drop_table_if_exists_silent_when_missing() {
        let (_, skips) = replay("DROP TABLE IF EXISTS nonexistent");
        assert!(skips.is_empty(), "got {skips:?}");
    }

    #[test]
    fn drop_table_missing_no_guard_emits_skip() {
        let (_, skips) = replay("DROP TABLE missing_one");
        assert!(matches!(
            skips.as_slice(),
            [SchemaSkip::DropMissingTable { .. }]
        ));
    }

    #[test]
    fn create_index_records_index() {
        let (s, _) = replay(
            "CREATE TABLE t (id INT, email VARCHAR(255)); \
             CREATE UNIQUE INDEX ix_email ON t (email)",
        );
        let t = s.table("t").unwrap();
        assert_eq!(t.indexes.len(), 1);
        assert_eq!(t.indexes[0].name.as_deref(), Some("ix_email"));
        assert!(t.indexes[0].unique);
    }

    #[test]
    fn alter_table_add_index_equivalent() {
        let (s, _) = replay(
            "CREATE TABLE t (id INT, email VARCHAR(255)); \
             ALTER TABLE t ADD UNIQUE INDEX ix_email (email)",
        );
        let t = s.table("t").unwrap();
        assert_eq!(t.indexes.len(), 1);
        assert!(t.indexes[0].unique);
    }

    #[test]
    fn add_constraint_foreign_key_resolves_via_cursor() {
        let (s, _) = replay(
            "USE shop_db; \
             CREATE TABLE users (id INT NOT NULL); \
             CREATE TABLE orders (id INT NOT NULL, user_id INT, \
                                  CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users (id))",
        );
        let orders = s.table_qualified("shop_db", "orders").unwrap();
        assert_eq!(orders.foreign_keys.len(), 1);
        assert_eq!(orders.foreign_keys[0].ref_db, "shop_db");
        assert_eq!(orders.foreign_keys[0].ref_table, "users");
    }

    #[test]
    fn use_database_changes_cursor() {
        let (s, _) = replay("USE shop_db; CREATE TABLE foo (id INT)");
        let t = s.table_qualified("shop_db", "foo").unwrap();
        assert_eq!(t.database, "shop_db");
    }

    #[test]
    fn same_table_in_two_dbs_does_not_collide() {
        let (s, _) = replay(
            "USE shop_db; CREATE TABLE orders (sid INT); \
             USE global_db; CREATE TABLE orders (gid INT)",
        );
        let shop = s.table_qualified("shop_db", "orders").unwrap();
        let global = s.table_qualified("global_db", "orders").unwrap();
        assert!(shop.columns.contains_key("sid"));
        assert!(!shop.columns.contains_key("gid"));
        assert!(global.columns.contains_key("gid"));
        assert!(!global.columns.contains_key("sid"));
    }

    #[test]
    fn duplicate_create_warns_and_overwrites() {
        let (s, skips) = replay(
            "CREATE TABLE t (a INT); \
             CREATE TABLE t (b INT)",
        );
        assert!(matches!(
            skips.as_slice(),
            [SchemaSkip::DuplicateTable { .. }]
        ));
        let t = s.table("t").unwrap();
        assert!(t.columns.contains_key("b"));
        assert!(!t.columns.contains_key("a"));
    }

    #[test]
    fn create_table_if_not_exists_no_overwrite() {
        let (s, _) = replay(
            "CREATE TABLE t (a INT); \
             CREATE TABLE IF NOT EXISTS t (b INT)",
        );
        let t = s.table("t").unwrap();
        assert!(t.columns.contains_key("a"));
        assert!(!t.columns.contains_key("b"));
    }

    #[test]
    fn unknown_kind_emits_skip() {
        let (_, skips) = replay("INSERT INTO t VALUES (1)");
        assert!(
            skips
                .iter()
                .any(|s| matches!(s, SchemaSkip::Unknown { kind, .. } if kind == "Insert"))
        );
    }

    #[test]
    fn drop_column_also_strips_from_pk_and_indexes() {
        let (s, _) = replay(
            "CREATE TABLE t (a INT NOT NULL, b INT NOT NULL, PRIMARY KEY (a, b)); \
             ALTER TABLE t DROP COLUMN b",
        );
        let t = s.table("t").unwrap();
        assert_eq!(t.primary_key, vec!["a".to_string()]);
    }
}
