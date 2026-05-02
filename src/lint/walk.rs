//! Shared AST traversal driver for the lint pass.
//!
//! sqlparser's `Visitor` only fires on `Statement`, `Query`, `Expr`,
//! `TableFactor`, `ObjectName`, and `Value`. Rules that operate at the
//! `Select` level (the unit most join/where/projection rules want) need a
//! manual descent — that's what this module provides. We use the upstream
//! `Visit` framework to discover queries inside a statement, then explicitly
//! descend into each query's body to fire `check_select` and walk the
//! `TableWithJoins` list.

use std::ops::ControlFlow;

use sqlparser::ast::{Expr, Query, Select, SetExpr, Statement, Visit, Visitor};
use sqlparser::tokenizer::Span;

use crate::ast::SqltStatement;
use crate::lint::ctx::LintCtx;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::Rule;

pub fn walk_statement(
    stmt: &SqltStatement,
    rules: &[Box<dyn Rule>],
    ctx: &LintCtx,
    out: &mut Vec<Diagnostic>,
) {
    // 1. Statement-level pass — both Std and Raw flow through here so rules
    //    like raw-passthrough can fire.
    for r in rules {
        r.check_statement(stmt, ctx, out);
    }

    // 2. Std statements get the AST walk.
    let SqltStatement::Std(boxed) = stmt else {
        return;
    };
    let mut driver = Driver {
        rules,
        ctx,
        out,
        query_depth: 0,
    };
    let _ = (**boxed).visit(&mut driver);
}

struct Driver<'a, 'b, 'c> {
    rules: &'a [Box<dyn Rule>],
    ctx: &'a LintCtx<'b>,
    out: &'c mut Vec<Diagnostic>,
    /// 0 for the outermost statement-level Query; increases as we descend
    /// into nested queries.
    query_depth: usize,
}

impl<'a, 'b, 'c> Driver<'a, 'b, 'c> {
    fn fire_query(&mut self, query: &Query) {
        for r in self.rules {
            r.check_query(query, self.query_depth, self.ctx, self.out);
        }
        // Descend into the query body looking for Selects. SetExpr can be
        // a Select, a SetOperation (UNION/INTERSECT/EXCEPT), or a parenthesized
        // Query — recurse through unions and parens, run check_select on selects.
        self.descend_set_expr(&query.body);
    }

    fn descend_set_expr(&mut self, set: &SetExpr) {
        match set {
            SetExpr::Select(s) => self.fire_select(s),
            SetExpr::Query(q) => {
                self.query_depth += 1;
                for r in self.rules {
                    r.check_query(q, self.query_depth, self.ctx, self.out);
                }
                self.descend_set_expr(&q.body);
                self.query_depth -= 1;
            }
            SetExpr::SetOperation { left, right, .. } => {
                self.descend_set_expr(left);
                self.descend_set_expr(right);
            }
            SetExpr::Values(_)
            | SetExpr::Insert(_)
            | SetExpr::Update(_)
            | SetExpr::Delete(_)
            | SetExpr::Merge(_)
            | SetExpr::Table(_) => {}
        }
    }

    fn fire_select(&mut self, select: &Select) {
        for r in self.rules {
            r.check_select(select, self.ctx, self.out);
        }
    }
}

impl Visitor for Driver<'_, '_, '_> {
    type Break = ();

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<Self::Break> {
        self.fire_query(query);
        // Children of this query (subqueries inside its body) are nested.
        self.query_depth += 1;
        ControlFlow::Continue(())
    }

    fn post_visit_query(&mut self, _query: &Query) -> ControlFlow<Self::Break> {
        self.query_depth = self.query_depth.saturating_sub(1);
        ControlFlow::Continue(())
    }

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<Self::Break> {
        for r in self.rules {
            r.check_expr(expr, self.ctx, self.out);
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_statement(&mut self, _stmt: &Statement) -> ControlFlow<Self::Break> {
        // Statement-level rules already fired in walk_statement; nothing here.
        ControlFlow::Continue(())
    }
}

/// Compute a best-effort span for a statement. Falls back to `Span::empty()`
/// for variants whose upstream `Spanned` impl is unimplemented.
pub fn statement_span(stmt: &Statement) -> Span {
    use sqlparser::ast::Spanned;
    stmt.span()
}
