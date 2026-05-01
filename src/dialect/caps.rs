//! Per-dialect capability tables.
//!
//! These tables drive the translation rewriter (`crate::translate::rewrite`).
//! When a node uses a capability the source dialect has but the target lacks,
//! the rewriter either drops the node, rewrites it to an equivalent, or
//! leaves it alone with a warning — see `WarnCode` for the full catalog.
//!
//! Add new fields here and a new `WarnCode` variant whenever the rewriter
//! needs a new translation rule.

use crate::dialect::DialectId;

#[derive(Debug, Clone, Copy)]
pub struct DialectCaps {
    /// `INSERT ... RETURNING <select_items>`
    pub returning_in_insert: bool,
    /// `UPDATE ... RETURNING <select_items>`
    pub returning_in_update: bool,
    /// `DELETE ... RETURNING <select_items>`
    pub returning_in_delete: bool,
    /// `CREATE [OR REPLACE] [TEMPORARY] SEQUENCE`
    pub create_sequence: bool,
    /// MySQL/MariaDB `INSERT ... ON DUPLICATE KEY UPDATE`
    pub on_duplicate_key_update: bool,
    /// PostgreSQL/SQLite `INSERT ... ON CONFLICT ...`
    pub on_conflict: bool,
    /// Whether the dialect can carry MariaDB raw fallback constructs as-is.
    /// Only MariaDB sets this — every other target needs a warning when a
    /// raw fragment is encountered.
    pub mariadb_raw_native: bool,
}

const NONE: DialectCaps = DialectCaps {
    returning_in_insert: false,
    returning_in_update: false,
    returning_in_delete: false,
    create_sequence: false,
    on_duplicate_key_update: false,
    on_conflict: false,
    mariadb_raw_native: false,
};

pub const MYSQL: DialectCaps = DialectCaps {
    on_duplicate_key_update: true,
    ..NONE
};

pub const MARIADB: DialectCaps = DialectCaps {
    returning_in_insert: true,
    returning_in_update: true,
    returning_in_delete: true,
    create_sequence: true,
    on_duplicate_key_update: true,
    mariadb_raw_native: true,
    ..NONE
};

pub const POSTGRES: DialectCaps = DialectCaps {
    returning_in_insert: true,
    returning_in_update: true,
    returning_in_delete: true,
    create_sequence: true,
    on_conflict: true,
    ..NONE
};

pub const MSSQL: DialectCaps = DialectCaps {
    create_sequence: true,
    ..NONE
};

pub const SQLITE: DialectCaps = DialectCaps {
    returning_in_insert: true,
    returning_in_update: true,
    returning_in_delete: true,
    on_conflict: true,
    ..NONE
};

pub const GENERIC: DialectCaps = DialectCaps {
    returning_in_insert: true,
    returning_in_update: true,
    returning_in_delete: true,
    create_sequence: true,
    on_duplicate_key_update: true,
    on_conflict: true,
    ..NONE
};

pub fn caps_for(dialect: DialectId) -> DialectCaps {
    match dialect {
        DialectId::MySql => MYSQL,
        DialectId::MariaDb => MARIADB,
        DialectId::Postgres => POSTGRES,
        DialectId::MsSql => MSSQL,
        DialectId::Sqlite => SQLITE,
        DialectId::Generic => GENERIC,
    }
}
