use serde_json::Value;
use std::path::{Path, PathBuf};

const ROOT_ENTRYPOINT: &str = "packaging/omarchy/omastat/BarWidget.qml";
const PACKAGED_ENTRYPOINT: &str = "BarWidget.qml";

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
