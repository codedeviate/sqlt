pub mod mariadb;

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
    pub fn upstream(self) -> Box<dyn Dialect> {
        match self {
            DialectId::MySql => Box::new(MySqlDialect {}),
            DialectId::MariaDb => Box::new(mariadb::MariaDbDialect::new()),
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
