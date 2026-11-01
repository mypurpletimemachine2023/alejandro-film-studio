//! A recovery checkpoint is one validated file, so metadata cannot refer to other project bytes.

use serde_json::{Value, json};

pub const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug)]
pub struct Record {
    pub name: String,
    pub native: Vec<u8>,
    pub dirty: bool,
    pub at: f64,
}

pub fn encode(native: &[u8], dirty: bool, at: f64) -> Result<Vec<u8>, String> {
    if native.len() > MAX_BYTES || !at.is_finite() || at < 0.0 {
        return Err("Recovery checkpoint is too large or has an invalid time".into());
    }
    let loaded = filmcraft_format::decode(native).map_err(|e| format!("Invalid recovery project: {e}"))?;
    let project: Value = serde_json::from_slice(native).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(&json!({"format":"aca.film.recovery","version":1,"name":loaded.project.name,"dirty":dirty,"at":at,"native":project}))
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("Recovery checkpoint exceeds 64 MiB; download the project".into());
    }
    Ok(bytes)
}

pub fn decode(bytes: &[u8]) -> Result<Record, String> {
    if bytes.len() > MAX_BYTES {
        return Err("Recovery checkpoint exceeds 64 MiB".into());
    }
    let v: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if v.get("format").and_then(Value::as_str) != Some("aca.film.recovery") || v.get("version").and_then(Value::as_u64) != Some(1) {
        return Err("Unsupported recovery checkpoint".into());
    }
    let at = v.get("at").and_then(Value::as_f64).filter(|v| v.is_finite() && *v >= 0.0).ok_or("Invalid recovery time")?;
    let dirty = v.get("dirty").and_then(Value::as_bool).ok_or("Missing recovery state")?;
    let native = serde_json::to_vec(v.get("native").ok_or("Missing recovery project")?).map_err(|e| e.to_string())?;
    let loaded = filmcraft_format::decode(&native).map_err(|e| format!("Invalid recovery project: {e}"))?;
    // Name comes from the validated native document, never a separate untrusted binding.
    Ok(Record { name: loaded.project.name, native, dirty, at })
}

/// Ignore corrupt/missing candidates while keeping the most recent valid clean or dirty project.
pub fn latest(candidates: impl IntoIterator<Item = Vec<u8>>) -> Option<Record> {
    candidates.into_iter().filter_map(|b| decode(&b).ok()).max_by(|a, b| a.at.total_cmp(&b.at))
}
