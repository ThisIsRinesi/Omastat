use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const PLUGIN_ID: &str = "local.omastat";
const ROOT_ENTRYPOINT: &str = "packaging/omarchy/omastat/BarWidget.qml";
const PACKAGED_ENTRYPOINT: &str = "BarWidget.qml";
const REQUIRED_PACKAGED_FILES: &[&str] =
    &["manifest.json", "BarWidget.qml", "Panel.qml", "Model.js"];

#[test]
fn omarchy_manifests_stay_synchronized() {
    let workspace = workspace_root();
    let root_manifest_path = workspace.join("manifest.json");
    let packaged_manifest_path = workspace.join("packaging/omarchy/omastat/manifest.json");

    let mut root = read_manifest(&root_manifest_path);
    let mut packaged = read_manifest(&packaged_manifest_path);

    assert_eq!(
        bar_widget_entrypoint(&root),
        ROOT_ENTRYPOINT,
        "root manifest entryPoints.barWidget should point at the packaged widget path"
    );
    assert_eq!(
        bar_widget_entrypoint(&packaged),
        PACKAGED_ENTRYPOINT,
        "packaged manifest entryPoints.barWidget should be local to the packaged plugin directory"
    );
    assert_entrypoint_exists(&workspace, &root_manifest_path, &root);
    assert_entrypoint_exists(
        &workspace.join("packaging/omarchy/omastat"),
        &packaged_manifest_path,
        &packaged,
    );

    set_bar_widget_entrypoint(&mut root, "<bar-widget>");
    set_bar_widget_entrypoint(&mut packaged, "<bar-widget>");

    assert_eq!(
        root, packaged,
        "root manifest and packaged manifest must match except for entryPoints.barWidget"
    );
}

#[test]
fn packaged_plugin_data_matches_omarchy_bar_widget_contract() {
    let workspace = workspace_root();
    let plugin_dir = workspace.join("packaging/omarchy/omastat");
    let manifest_path = plugin_dir.join("manifest.json");
    let manifest = read_manifest(&manifest_path);

    assert_eq!(
        number_at(&manifest, "/schemaVersion"),
        1,
        "Omarchy only accepts schemaVersion 1"
    );
    assert_eq!(
        string_at(&manifest, "/id"),
        PLUGIN_ID,
        "plugin id must stay stable so existing bar layouts keep updating this widget"
    );
    assert_eq!(
        string_at(&manifest, "/entryPoints/barWidget"),
        PACKAGED_ENTRYPOINT,
        "packaged plugin data must point at the local widget file"
    );
    assert!(
        array_at(&manifest, "/kinds")
            .iter()
            .any(|kind| kind.as_str() == Some("bar-widget")),
        "plugin kinds must include bar-widget so Omarchy registers it with the bar"
    );
    assert_eq!(
        string_at(&manifest, "/barWidget/displayName"),
        "Omastat",
        "display name is what Omarchy shows in widget pickers"
    );
    assert_eq!(
        string_at(&manifest, "/barWidget/defaultSection"),
        "right",
        "default section should keep the analytics widget near other status widgets"
    );
    assert!(
        !bool_at(&manifest, "/barWidget/allowMultiple"),
        "the widget owns fixed caches and settings, so multiple instances should remain disabled"
    );

    assert_schema_is_unique_and_matches_defaults(&manifest);
    assert_required_packaged_files_exist(&plugin_dir);
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/omastat")
        .to_path_buf()
}

fn read_manifest(path: &Path) -> Value {
    let contents = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn bar_widget_entrypoint(manifest: &Value) -> &str {
    manifest
        .pointer("/entryPoints/barWidget")
        .and_then(Value::as_str)
        .expect("manifest should define entryPoints.barWidget")
}

fn set_bar_widget_entrypoint(manifest: &mut Value, value: &str) {
    *manifest
        .pointer_mut("/entryPoints/barWidget")
        .expect("manifest should define entryPoints.barWidget") = Value::String(value.to_string());
}

fn assert_entrypoint_exists(base_dir: &Path, manifest_path: &Path, manifest: &Value) {
    let entrypoint = bar_widget_entrypoint(manifest);
    let entrypoint_path = Path::new(entrypoint);
    assert!(
        !entrypoint_path.is_absolute(),
        "{} entryPoints.barWidget must be a relative path",
        manifest_path.display()
    );
    assert!(
        !entrypoint_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir)),
        "{} entryPoints.barWidget must not contain '..'",
        manifest_path.display()
    );

    let resolved = base_dir.join(entrypoint_path);
    assert!(
        resolved.is_file(),
        "{} entryPoints.barWidget points at missing file {}",
        manifest_path.display(),
        resolved.display()
    );
}

fn value_at<'a>(manifest: &'a Value, pointer: &str) -> &'a Value {
    manifest
        .pointer(pointer)
        .unwrap_or_else(|| panic!("manifest should define {pointer}"))
}

fn string_at<'a>(manifest: &'a Value, pointer: &str) -> &'a str {
    value_at(manifest, pointer)
        .as_str()
        .unwrap_or_else(|| panic!("manifest {pointer} should be a string"))
}

fn number_at(manifest: &Value, pointer: &str) -> i64 {
    value_at(manifest, pointer)
        .as_i64()
        .unwrap_or_else(|| panic!("manifest {pointer} should be an integer"))
}

fn bool_at(manifest: &Value, pointer: &str) -> bool {
    value_at(manifest, pointer)
        .as_bool()
        .unwrap_or_else(|| panic!("manifest {pointer} should be a boolean"))
}

fn array_at<'a>(manifest: &'a Value, pointer: &str) -> &'a Vec<Value> {
    value_at(manifest, pointer)
        .as_array()
        .unwrap_or_else(|| panic!("manifest {pointer} should be an array"))
}

fn object_at<'a>(manifest: &'a Value, pointer: &str) -> &'a serde_json::Map<String, Value> {
    value_at(manifest, pointer)
        .as_object()
        .unwrap_or_else(|| panic!("manifest {pointer} should be an object"))
}

fn assert_schema_is_unique_and_matches_defaults(manifest: &Value) {
    let defaults = object_at(manifest, "/barWidget/defaults");
    let schema = array_at(manifest, "/barWidget/schema");
    let mut keys = HashSet::new();

    assert!(
        defaults.contains_key("refreshIntervalSec"),
        "barWidget.defaults must include refreshIntervalSec"
    );
    assert!(
        defaults.contains_key("iconOnly"),
        "barWidget.defaults must include iconOnly"
    );

    for field in schema {
        let key = field
            .get("key")
            .and_then(Value::as_str)
            .expect("each barWidget.schema item should have a string key");
        assert!(
            keys.insert(key.to_string()),
            "barWidget.schema contains duplicate key {key}"
        );
        let default_value = field
            .get("defaultValue")
            .unwrap_or_else(|| panic!("schema item {key} should define defaultValue"));
        assert_eq!(
            defaults.get(key),
            Some(default_value),
            "schema defaultValue for {key} should match barWidget.defaults"
        );
        assert_schema_field_is_well_formed(key, field);
    }

    for key in defaults.keys() {
        assert!(
            keys.contains(key),
            "barWidget.defaults key {key} should have a matching schema entry"
        );
    }
}

fn assert_schema_field_is_well_formed(key: &str, field: &Value) {
    let object = field
        .as_object()
        .expect("barWidget.schema entries should be objects");
    let field_type = object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("schema item {key} should define type"));
    assert!(
        object.get("label").and_then(Value::as_str).is_some(),
        "schema item {key} should define a label"
    );

    match field_type {
        "boolean" => {
            assert!(
                object
                    .get("defaultValue")
                    .and_then(Value::as_bool)
                    .is_some(),
                "boolean schema item {key} should have a boolean defaultValue"
            );
        }
        "integer" => {
            let default = object
                .get("defaultValue")
                .and_then(Value::as_i64)
                .unwrap_or_else(|| {
                    panic!("integer schema item {key} should have integer defaultValue")
                });
            let min = object
                .get("min")
                .and_then(Value::as_i64)
                .unwrap_or_else(|| panic!("integer schema item {key} should define min"));
            let max = object
                .get("max")
                .and_then(Value::as_i64)
                .unwrap_or_else(|| panic!("integer schema item {key} should define max"));
            let step = object
                .get("step")
                .and_then(Value::as_i64)
                .unwrap_or_else(|| panic!("integer schema item {key} should define step"));
            assert!(
                min <= default && default <= max,
                "integer schema item {key} defaultValue should be within min/max"
            );
            assert!(
                step > 0,
                "integer schema item {key} step should be positive"
            );
            assert_eq!(
                (default - min) % step,
                0,
                "integer schema item {key} defaultValue should align with step"
            );
        }
        other => panic!("unsupported barWidget.schema type {other} for {key}"),
    }
}

fn assert_required_packaged_files_exist(plugin_dir: &Path) {
    for file in REQUIRED_PACKAGED_FILES {
        let path = plugin_dir.join(file);
        assert!(
            path.is_file(),
            "packaged plugin is missing required runtime file {}",
            path.display()
        );
    }
}
