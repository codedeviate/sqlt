//! Rule registry. Owns the `Vec<Box<dyn Rule>>` and resolves
//! `--rule`/`--no-rule` selections by id, short numeric id, or slug.

use std::collections::BTreeMap;

use crate::error::{Error, Result};
use crate::lint::rule::{Rule, RuleId, RuleMeta};
use crate::lint::rules;

/// Build the canonical list of all registered rules. Order is the iteration
/// order for diagnostics from a single statement; sort by rule id to keep
/// output stable.
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    let mut v: Vec<Box<dyn Rule>> = vec![
        // raw passthrough
        Box::new(rules::raw::RawPassthrough),
        // dialect cross-contamination
        Box::new(rules::dialect_xc::MysqlBacktickInNonMysql),
        Box::new(rules::dialect_xc::MssqlBracketInNonMssql),
        Box::new(rules::dialect_xc::PostgresDoubleColonCastInMysql),
        Box::new(rules::dialect_xc::MysqlOnDuplicateKeyInNonMysql),
        Box::new(rules::dialect_xc::ReturningInMysql),
        // pre-flight (uses --to)
        Box::new(rules::pre_flight::PreflightReturningUnsupported),
        Box::new(rules::pre_flight::PreflightOnDuplicateUnsupported),
        Box::new(rules::pre_flight::PreflightOnConflictUnsupported),
        Box::new(rules::pre_flight::PreflightCreateSequenceUnsupported),
        Box::new(rules::pre_flight::PreflightRawPassthroughUnsupported),
        Box::new(rules::pre_flight::PreflightQuoteStyleMismatch),
        // perf
        Box::new(rules::perf::SelectStar),
        Box::new(rules::perf::SelectStarQualified),
        Box::new(rules::perf::LeadingWildcardLike),
        Box::new(rules::perf::FunctionOnColumnInWhere),
        Box::new(rules::perf::DistinctAsJoinFix),
        Box::new(rules::perf::CountOfNullableColumn),
        Box::new(rules::perf::ImplicitStringNumericCompare),
        Box::new(rules::perf::OrAcrossColumns),
        // joins
        Box::new(rules::joins::ImplicitCrossJoin),
        Box::new(rules::joins::CrossJoinWithoutWhere),
        Box::new(rules::joins::NaturalJoin),
        Box::new(rules::joins::JoinWithoutOn),
        Box::new(rules::joins::OnTautology),
        Box::new(rules::joins::UsingWithQuotedIdent),
        Box::new(rules::joins::FullOuterMysql),
        Box::new(rules::joins::CommaJoinWithOnElsewhere),
        // correctness
        Box::new(rules::correctness::EqualsNull),
        Box::new(rules::correctness::UpdateWithoutWhere),
        Box::new(rules::correctness::DeleteWithoutWhere),
        Box::new(rules::correctness::MixedAndOrNoParens),
        Box::new(rules::correctness::OrderByPositional),
        Box::new(rules::correctness::HavingWithoutGroupBy),
        Box::new(rules::correctness::GroupByPositional),
        // subquery
        Box::new(rules::subquery::NotInSubqueryNullPitfall),
        Box::new(rules::subquery::InSubqueryPreferExists),
        Box::new(rules::subquery::ScalarSubqueryInSelect),
        Box::new(rules::subquery::OrderByInSubqueryWithoutLimit),
        Box::new(rules::subquery::CorrelatedSubqueryInWhere),
        // style
        Box::new(rules::style::UnaliasedDerivedTable),
        Box::new(rules::style::NonDeterministicPagination),
        // ddl
        Box::new(rules::ddl::FloatForMoney),
        Box::new(rules::ddl::VarcharWithoutLength),
        // schema-aware
        Box::new(rules::schema_aware::UnknownColumn),
    ];
    v.sort_by_key(|r| r.meta().id);
    v
}

/// Apply `--rule` / `--no-rule` selectors (in CLI order) to a default-enabled
/// map. Returns the filtered set of rules to run.
pub fn select_rules(enable: &[String], disable: &[String]) -> Result<Vec<Box<dyn Rule>>> {
    let all = all_rules();
    let lookup = build_lookup(&all);

    let mut state: BTreeMap<RuleId, bool> = all
        .iter()
        .map(|r| (r.meta().id, r.meta().default_enabled))
        .collect();

    // Disables come before enables on the CLI; clap doesn't preserve
    // interleaved order across distinct flags by default. Apply disables
    // first, then enables — so `--no-rule X --rule X` ends up enabled, which
    // matches the typical "enable specific rule" intent.
    for s in disable {
        let id = resolve_id(s, &lookup)?;
        state.insert(id, false);
    }
    for s in enable {
        let id = resolve_id(s, &lookup)?;
        state.insert(id, true);
    }

    Ok(all.into_iter().filter(|r| state[&r.meta().id]).collect())
}

fn build_lookup(rules: &[Box<dyn Rule>]) -> BTreeMap<String, RuleId> {
    let mut m = BTreeMap::new();
    for r in rules {
        let meta = r.meta();
        m.insert(meta.id.as_str().to_string(), meta.id);
        m.insert(meta.name.to_string(), meta.id);
        // Numeric-only short form: "SQLT0500" → "0500" → "500".
        if let Some(num) = meta.id.as_str().strip_prefix("SQLT") {
            m.insert(num.to_string(), meta.id);
            if let Some(stripped) = num.trim_start_matches('0').strip_prefix("") {
                if !stripped.is_empty() {
                    m.insert(stripped.to_string(), meta.id);
                }
            }
        }
    }
    m
}

fn resolve_id(s: &str, lookup: &BTreeMap<String, RuleId>) -> Result<RuleId> {
    if let Some(id) = lookup.get(s) {
        return Ok(*id);
    }
    Err(Error::UnknownRule(s.to_string()))
}

/// Look up a rule's metadata by any accepted form (full id, short numeric, slug).
pub fn find_meta(s: &str) -> Result<&'static RuleMeta> {
    let all = all_rules();
    let lookup = build_lookup(&all);
    let id = resolve_id(s, &lookup)?;
    all.iter()
        .find(|r| r.meta().id == id)
        .map(|r| r.meta())
        .ok_or_else(|| Error::UnknownRule(s.to_string()))
}
