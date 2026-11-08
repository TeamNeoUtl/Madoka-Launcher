use launcher_providers::parse_aviutl2_manifest;

fn manifest(extra: &str) -> String {
    format!(
        r#"{{"id":"a.b","name":"N","type":"plugin","summary":"s","author":"me",
        "repoURL":"https://github.com/o/r","latest-version":"1.0",
        "installer":{{"source":{{"direct":"https://example.com/a.zip"}},"install":[]}}{extra}}}"#
    )
}

#[test]
fn commons_id_is_extracted_verbatim() {
    let package = parse_aviutl2_manifest(&manifest(r#","niconiCommonsId":"nc123456""#)).unwrap();
    assert_eq!(package.niconi_commons_id.as_deref(), Some("nc123456"));
}

#[test]
fn commons_id_is_absent_by_default() {
    let package = parse_aviutl2_manifest(&manifest("")).unwrap();
    assert_eq!(package.niconi_commons_id, None);
}
