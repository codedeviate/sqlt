//! Schema-aware rules — only fire when a `CREATE TABLE` for the
//! referenced object is present in the same input.

use sqlparser::ast::{Expr, ObjectNamePart, Select, SelectItem, Statement, TableFactor};

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
        // INSERT INTO Foo (col1, col2, …) — column list is unqualified but
        // the target table is known.
        if let Statement::Insert(i) = &**boxed {
            let table = match &i.table {
                sqlparser::ast::TableObject::TableName(name) => last_part(name.0.as_slice()),
                _ => None,
            };
            if let Some(table) = table
                && ctx.schema.table(&table).is_some()
            {
                for col in &i.columns {
                    if ctx.schema.column(&table, &col.value).is_none() {
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

/// Build the set of identifiers visible as table-side names in a SELECT —
/// either the explicit alias (`users u`) or the table name itself
/// (`users`). Maps each to the underlying schema-table-name (lower-cased).
/// Skips entries we can't resolve (CTEs, derived tables, function calls).
fn collect_known_aliases(select: &Select, ctx: &LintCtx) -> Vec<(String, String)> {
    let mut aliases = Vec::new();
    for tw in &select.from {
        push_table_factor(&tw.relation, ctx, &mut aliases);
        for j in &tw.joins {
            push_table_factor(&j.relation, ctx, &mut aliases);
        }
    }
    aliases
}

fn push_table_factor(tf: &TableFactor, ctx: &LintCtx, aliases: &mut Vec<(String, String)>) {
    if let TableFactor::Table { name, alias, .. } = tf {
        let Some(schema_name) = last_part(name.0.as_slice()) else {
            return;
        };
        if ctx.schema.table(&schema_name).is_none() {
            // Unknown table — don't claim it.
            return;
        }
        let visible = match alias {
            Some(a) => a.name.value.clone(),
            None => schema_name.clone(),
        };
        aliases.push((visible, schema_name));
    }
}

fn walk_expr(e: &Expr, aliases: &[(String, String)], ctx: &LintCtx, out: &mut Vec<Diagnostic>) {
    match e {
        Expr::CompoundIdentifier(parts) if parts.len() >= 2 => {
            let table_ref = &parts[parts.len() - 2].value;
            let col_ref = &parts[parts.len() - 1].value;
            // Resolve through the alias map.
            let schema_name = aliases
                .iter()
                .find(|(visible, _)| visible.eq_ignore_ascii_case(table_ref))
                .map(|(_, real)| real.clone());
            let Some(schema_name) = schema_name else {
                return;
            };
            if ctx.schema.column(&schema_name, col_ref).is_none() {
                out.push(diag(
                    &META_UNKNOWN_COL,
                    ctx,
                    &format!(
                        "column `{}` does not exist on table `{}` (declared in this input)",
                        col_ref, schema_name
                    ),
                    Some(format!(
                        "check spelling, or add `{}` to `CREATE TABLE {}`",
                        col_ref, schema_name
                    )),
                    parts[parts.len() - 1].span,
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

fn last_part(parts: &[ObjectNamePart]) -> Option<String> {
    parts.last().and_then(|p| match p {
        ObjectNamePart::Identifier(i) => Some(i.value.clone()),
        _ => None,
    })
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
