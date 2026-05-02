pub mod caps;

use std::str::FromStr;

use sqlparser::dialect::{
    Dialect, GenericDialect, MsSqlDialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect,
};

use crate::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DialectId {
    MySql,
    MariaDb,
    Postgres,
    MsSql,
    Sqlite,
    Generic,
}

impl DialectId {
    /// Map a `DialectId` to the upstream `sqlparser` dialect used to parse it.
    ///
    /// MariaDB intentionally returns `MySqlDialect`. We previously used a
    /// thin `MariaDbDialect` wrapper, but sqlparser gates many MySQL-only
    /// features (`ON UPDATE` timestamp column option, table hints, `LIMIT a, b`
    /// shorthand, etc.) behind `dialect_of!(MySqlDialect)` macros that
    /// downcast via `Any`. Any wrapper type, however faithful in trait
    /// forwarding, fails those `is::<MySqlDialect>()` checks and silently
    /// loses dozens of MySQL-superset features that MariaDB needs. Using
    /// `MySqlDialect` directly is the only reliable way to get the full
    /// MariaDB grammar parsed today; the application-level distinction is
    /// preserved via this `DialectId` enum.
    ///
    /// MariaDB input is pre-processed by `parse::parse` to handle the bare
    /// `--<EOL>` comment form that real `mariadb-dump` output contains.
    pub fn upstream(self) -> Box<dyn Dialect> {
        match self {
            DialectId::MySql => Box::new(MySqlDialect {}),
            DialectId::MariaDb => Box::new(MySqlDialect {}),
            DialectId::Postgres => Box::new(PostgreSqlDialect {}),
            DialectId::MsSql => Box::new(MsSqlDialect {}),
            DialectId::Sqlite => Box::new(SQLiteDialect {}),
            DialectId::Generic => Box::new(GenericDialect {}),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            DialectId::MySql => "mysql",
            DialectId::MariaDb => "mariadb",
            DialectId::Postgres => "postgres",
            DialectId::MsSql => "mssql",
            DialectId::Sqlite => "sqlite",
            DialectId::Generic => "generic",
        }
    }
}

impl FromStr for DialectId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "mysql" => Ok(DialectId::MySql),
            "mariadb" | "maria" => Ok(DialectId::MariaDb),
            "postgres" | "postgresql" | "pg" => Ok(DialectId::Postgres),
            "mssql" | "tsql" | "sqlserver" => Ok(DialectId::MsSql),
            "sqlite" => Ok(DialectId::Sqlite),
            "generic" => Ok(DialectId::Generic),
            other => Err(Error::UnknownDialect(other.to_string())),
        }
    }
}

impl std::fmt::Display for DialectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
