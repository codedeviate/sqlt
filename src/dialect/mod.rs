use std::str::FromStr;

use sqlparser::dialect::{Dialect, GenericDialect, MySqlDialect};

use crate::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DialectId {
    MySql,
    Generic,
}

impl DialectId {
    pub fn upstream(self) -> Box<dyn Dialect> {
        match self {
            DialectId::MySql => Box::new(MySqlDialect {}),
            DialectId::Generic => Box::new(GenericDialect {}),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            DialectId::MySql => "mysql",
            DialectId::Generic => "generic",
        }
    }
}

impl FromStr for DialectId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "mysql" => Ok(DialectId::MySql),
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
