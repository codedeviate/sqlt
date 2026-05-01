pub mod envelope;

pub use envelope::Envelope;

use crate::error::Result;

pub fn serialize(env: &Envelope, pretty: bool) -> Result<String> {
    let s = if pretty {
        serde_json::to_string_pretty(env)?
    } else {
        serde_json::to_string(env)?
    };
    Ok(s)
}

pub fn deserialize(s: &str) -> Result<Envelope> {
    Ok(serde_json::from_str(s)?)
}
