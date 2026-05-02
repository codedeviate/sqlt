//! Schema-aware rules — only fire when a `CREATE TABLE` for the
//! referenced object is present in the same input.

use sqlparser::ast::{Expr, Select, SelectItem, Statement, TableFactor};

use crate::ast::SqltStatement;
use crate::lint::ctx::LintCtx;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Category, Rule, RuleId, RuleMeta, Severity};

// ───────────────────────────── SQLT0900 unknown-column ──────────────────────

pub struct UnknownColumn;

const META_UNKNOWN_COL: RuleMeta = RuleMeta {
    id: RuleId("SQLT0900"),
    name: "unknown-column",
    category: Category::Schema,
    default_severity: Severity::Error,
    default_enabled: true,
    summary: "A column reference of the form `table.col` does not exist on the table's CREATE TABLE.",
    explanation: "Schema-aware rule. Only fires when:\n  \
                  * the referenced table is declared via `CREATE TABLE` in the same input,\n  \
                  * the reference is qualified (`table.col`, not bare `col`),\n  \
                  * the named column is not present on that table.\n\
                  Conservative on purpose — it never warns about CTEs, derived-table aliases, \
                  or tables defined elsewhere. Catches typos and stale references in queries \
                  that ship alongside their schema.",
};

impl Rule for UnknownColumn {
    fn meta(&self) -> &'static RuleMeta {
        &META_UNKNOWN_COL
    }

    fn check_statement(&self, stmt: &SqltStatement, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if ctx.schema.is_empty() {
            return;
        }
        let SqltStatement::Std(boxed) = stmt else {
            return;
        };
        // INSERT INTO [db.]Foo (col1, col2, …) — column list is unqualified
        // but the target table can be qualified.
        if let Statement::Insert(i) = &**boxed {
            let resolved = match &i.table {
                sqlparser::ast::TableObject::TableName(name) => {
                    Some(ctx.schema.resolve_table_name(name))
                }
                _ => None,
            };
            if let Some((db, table)) = resolved
                && ctx.schema.table_qualified(&db, &table).is_some()
            {
                for col in &i.columns {
                    if ctx
                        .schema
                        .column_qualified(&db, &table, &col.value)
                        .is_none()
                    {
                        out.push(diag(
                            &META_UNKNOWN_COL,
                            ctx,
                            &format!(
                                "INSERT references column `{}` on table `{}`, but it is not declared in any `CREATE TABLE {}` in this input",
                                col.value, table, table
                            ),
                            Some(format!(
                                "check the spelling, or add `{}` to the `CREATE TABLE {}` definition",
                                col.value, table
                            )),
                            col.span,
                        ));
                    }
                }
            }
        }
    }

    fn check_select(&self, select: &Select, ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
        if ctx.schema.is_empty() {
            return;
        }
        // Build the alias→table map for the FROM clause: `users u` means
        // an `u.col` reference resolves through `users`.
        let known_aliases = collect_known_aliases(select, ctx);
        if known_aliases.is_empty() {
            return;
        }
        // Walk projection, selection, and JOIN ON predicates for compound
        // identifiers and check them against the schema.
        for item in &select.projection {
            match item {
                SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => {
                    walk_expr(e, &known_aliases, ctx, out);
                }
                _ => {}
            }
        }
        if let Some(sel) = &select.selection {
            walk_expr(sel, &known_aliases, ctx, out);
        }
        for tw in &select.from {
            for j in &tw.joins {
                if let sqlparser::ast::JoinOperator::Inner(c)
                | sqlparser::ast::JoinOperator::Join(c)
                | sqlparser::ast::JoinOperator::Left(c)
                | sqlparser::ast::JoinOperator::LeftOuter(c)
                | sqlparser::ast::JoinOperator::Right(c)
                | sqlparser::ast::JoinOperator::RightOuter(c)
                | sqlparser::ast::JoinOperator::FullOuter(c) = &j.join_operator
                    && let sqlparser::ast::JoinConstraint::On(e) = c
                {
                    walk_expr(e, &known_aliases, ctx, out);
                }
            }
        }
    }
}

/// One entry per table source visible inside a SELECT's FROM/JOIN. Maps
/// the user-visible alias (or bare table name) to the resolved
/// `(database, table)` pair from the schema.
#[derive(Clone, Debug)]
struct TableAlias {
    visible: String,
    database: String,
    table: String,
}

fn collect_known_aliases(select: &Select, ctx: &LintCtx) -> Vec<TableAlias> {
    let mut aliases = Vec::new();
    for tw in &select.from {
        push_table_factor(&tw.relation, ctx, &mut aliases);
        for j in &tw.joins {
            push_table_factor(&j.relation, ctx, &mut aliases);
        }
    }
    aliases
}

fn push_table_factor(tf: &TableFactor, ctx: &LintCtx, aliases: &mut Vec<TableAlias>) {
    if let TableFactor::Table { name, alias, .. } = tf {
        let (db, table) = ctx.schema.resolve_table_name(name);
        if ctx.schema.table_qualified(&db, &table).is_none() {
            return;
        }
        let visible = match alias {
            Some(a) => a.name.value.clone(),
            None => table.clone(),
        };
        aliases.push(TableAlias {
            visible,
            database: db,
            table,
        });
    }
}

fn walk_expr(e: &Expr, aliases: &[TableAlias], ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
    match e {
        Expr::CompoundIdentifier(parts) if parts.len() >= 2 => {
            let n = parts.len();
            let col_ref = &parts[n - 1].value;
            let col_span = parts[n - 1].span;

            // 3-part identifier: db.table.col
            if n >= 3 {
                let db_ref = &parts[n - 3].value;
                let table_ref = &parts[n - 2].value;
                if ctx.schema.table_qualified(db_ref, table_ref).is_some() {
                    if ctx
                        .schema
                        .column_qualified(db_ref, table_ref, col_ref)
                        .is_none()
                    {
                        out.push(diag(
                            &META_UNKNOWN_COL,
                            ctx,
                            &format!(
                                "column `{}` does not exist on table `{}.{}` (declared in this input)",
                                col_ref, db_ref, table_ref
                            ),
                            Some(format!(
                                "check spelling, or add `{}` to `CREATE TABLE {}.{}`",
                                col_ref, db_ref, table_ref
                            )),
                            col_span,
                        ));
                    }
                    return;
                }
                // 3-part but the table isn't in the schema — skip silently.
                return;
            }

            // 2-part identifier: alias.col or table.col
            let table_ref = &parts[n - 2].value;
            let alias = aliases
                .iter()
                .find(|a| a.visible.eq_ignore_ascii_case(table_ref));
            let Some(alias) = alias else {
                return;
            };
            if ctx
                .schema
                .column_qualified(&alias.database, &alias.table, col_ref)
                .is_none()
            {
                out.push(diag(
                    &META_UNKNOWN_COL,
                    ctx,
                    &format!(
                        "column `{}` does not exist on table `{}` (declared in this input)",
                        col_ref, alias.table
                    ),
                    Some(format!(
                        "check spelling, or add `{}` to `CREATE TABLE {}`",
                        col_ref, alias.table
                    )),
                    col_span,
                ));
            }
        }
        Expr::BinaryOp { left, right, .. } => {
            walk_expr(left, aliases, ctx, out);
            walk_expr(right, aliases, ctx, out);
        }
        Expr::UnaryOp { expr, .. } | Expr::Nested(expr) => walk_expr(expr, aliases, ctx, out),
        Expr::Like { expr, pattern, .. } | Expr::ILike { expr, pattern, .. } => {
            walk_expr(expr, aliases, ctx, out);
            walk_expr(pattern, aliases, ctx, out);
        }
        Expr::InList { expr, list, .. } => {
            walk_expr(expr, aliases, ctx, out);
            for x in list {
                walk_expr(x, aliases, ctx, out);
            }
        }
        Expr::Between {
            expr, low, high, ..
        } => {
            walk_expr(expr, aliases, ctx, out);
            walk_expr(low, aliases, ctx, out);
            walk_expr(high, aliases, ctx, out);
        }
        _ => {}
    }
}

fn diag(
    meta: &'static RuleMeta,
    ctx: &LintCtx,
    msg: &str,
    suggestion: Option<String>,
    span: sqlparser::tokenizer::Span,
) -> Diagnostic {
    Diagnostic {
        rule: meta.id,
        rule_name: meta.name,
        category: meta.category,
        severity: meta.default_severity,
        message: msg.to_string(),
        suggestion,
        span,
        stmt_index: ctx.stmt_index,
        source_dialect: ctx.src,
        target_dialect: ctx.dst,
    }
}
