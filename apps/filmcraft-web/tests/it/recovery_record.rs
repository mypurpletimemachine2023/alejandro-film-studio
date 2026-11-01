#[path = "../../src/recovery_record.rs"]
mod record;

fn project(name: &str) -> Vec<u8> {
    filmcraft_format::encode(&filmcraft_project::Project::new(name), false)
}

#[test]
fn saved_and_dirty_checkpoints_both_restore_exact_native_data() {
    for dirty in [false, true] {
        let native = project("Academy original");
        let encoded = record::encode(&native, dirty, 42.0).unwrap();
        let loaded = record::decode(&encoded).unwrap();
        assert_eq!(loaded.name, "Academy original");
        assert_eq!(loaded.dirty, dirty);
        assert_eq!(serde_json::from_slice::<serde_json::Value>(&loaded.native).unwrap(), serde_json::from_slice::<serde_json::Value>(&native).unwrap());
    }
}

#[test]
fn newest_valid_checkpoint_wins_without_losing_clean_save() {
    let saved = record::encode(&project("Saved"), false, 10.0).unwrap();
    let dirty = record::encode(&project("Dirty"), true, 20.0).unwrap();
    assert_eq!(record::latest([saved.clone(), dirty.clone()]).unwrap().name, "Dirty");
    assert_eq!(record::latest([saved.clone(), b"broken newest".to_vec()]).unwrap().name, "Saved");
    assert_eq!(record::latest([saved, dirty, record::encode(&project("Saved again"), false, 30.0).unwrap()]).unwrap().name, "Saved again");
    assert!(record::latest([b"corrupt".to_vec()]).is_none());
    assert!(record::latest(Vec::<Vec<u8>>::new()).is_none());
}

#[test]
fn corrupt_or_unsupported_metadata_never_installs_a_project() {
    let native = project("Kept");
    assert!(record::encode(b"not a project", false, 10.0).is_err());
    assert!(record::encode(&native, false, f64::NAN).is_err());
    let encoded = record::encode(&native, false, 10.0).unwrap();
    for (key, value) in [("version", serde_json::json!(500)), ("dirty", serde_json::Value::Null), ("native", serde_json::json!({}))] {
        let mut bad: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        bad[key] = value;
        assert!(record::decode(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
}

#[test]
fn oversized_recovery_record_is_refused_before_parsing() {
    let bytes = vec![b' '; record::MAX_BYTES + 1];
    assert!(record::decode(&bytes).is_err());
    assert!(record::encode(&bytes, false, 1.0).is_err());
}

#[test]
fn missing_media_reference_is_preserved_and_never_replaced_with_demo() {
    let legacy = include_bytes!("../../../../crates/format/tests/fixtures/v1-edit.fcproj");
    let mut native: serde_json::Value = serde_json::from_slice(legacy).unwrap();
    native["items"]["5"]["kind"]["Media"]["media"] = serde_json::json!({"File":{"path":"/files/owner-missing-original.mov"}});
    let native = serde_json::to_vec(&native).unwrap();
    let record = record::decode(&record::encode(&native, false, 10.0).unwrap()).unwrap();
    let loaded: serde_json::Value = serde_json::from_slice(&record.native).unwrap();
    assert_eq!(loaded["items"]["5"]["kind"]["Media"]["media"]["File"]["path"], "/files/owner-missing-original.mov");
}
