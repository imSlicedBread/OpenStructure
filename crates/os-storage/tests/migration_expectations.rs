mod common;

use serde_json::json;

#[test]
fn native_header_expectation_preserves_opaque_payloads_and_existing_hides() {
    let opaque = json!({"header":{"schema_version":73},"version":9,
        "parameters":{"schema_version":73,"plan_hidden_openings":{"keep":["opaque"]}}});
    let mut expected = json!({
        "schema_version":73,
        "project":{"header":{"schema_version":73,"properties":opaque}},
        "views":{"view":{"header":{"schema_version":73},"parameters":opaque}},
        "extensions":{"plugin":opaque}
    });
    common::current_native_headers(&mut expected);
    assert_eq!(expected["schema_version"], os_model::SCHEMA_VERSION);
    assert_eq!(
        expected["project"]["header"]["schema_version"],
        os_model::SCHEMA_VERSION
    );
    assert_eq!(
        expected["views"]["view"]["header"]["schema_version"],
        os_model::SCHEMA_VERSION
    );
    assert_eq!(expected["project"]["header"]["properties"], opaque);
    assert_eq!(expected["views"]["view"]["parameters"], opaque);
    assert_eq!(expected["extensions"]["plugin"], opaque);
    assert_eq!(expected["plan_hidden_openings"], json!({}));
    expected["plan_hidden_openings"] = json!({"view":["opening"]});
    let before = expected.clone();
    common::current_native_headers(&mut expected);
    assert_eq!(expected, before);
}
