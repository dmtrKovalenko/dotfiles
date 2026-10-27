//! `herdr-plugin.toml` is what Herdr reads; it must agree with the crate.

use std::path::Path;

use herdr_fingers::adapters::herdr::DEFAULT_PLUGIN_ID;
use herdr_fingers::usecases::start::OVERLAY_TITLE;

fn manifest() -> toml::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("herdr-plugin.toml");
    toml::from_str(&std::fs::read_to_string(path).expect("manifest exists")).expect("valid TOML")
}

#[test]
fn the_manifest_and_the_crate_share_one_version_and_one_id() {
    let manifest = manifest();
    assert_eq!(
        manifest["version"].as_str(),
        Some(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(manifest["id"].as_str(), Some(DEFAULT_PLUGIN_ID));
}

#[test]
fn the_overlay_pane_carries_the_title_the_start_action_looks_for() {
    let manifest = manifest();
    let panes = manifest["panes"].as_array().expect("panes");
    let overlay = panes
        .iter()
        .find(|pane| pane["id"].as_str() == Some("overlay"))
        .expect("an overlay pane");
    assert_eq!(overlay["title"].as_str(), Some(OVERLAY_TITLE));
    assert_eq!(overlay["placement"].as_str(), Some("overlay"));
    assert_eq!(
        overlay["command"][1].as_str(),
        Some("ui"),
        "the overlay pane must run the `ui` subcommand"
    );
}

#[test]
fn the_start_action_runs_the_release_binary() {
    let manifest = manifest();
    let actions = manifest["actions"].as_array().expect("actions");
    let start = actions
        .iter()
        .find(|action| action["id"].as_str() == Some("start"))
        .expect("a start action");
    assert_eq!(
        start["command"][0].as_str(),
        Some("./target/release/herdr-fingers")
    );
    assert_eq!(start["command"][1].as_str(), Some("start"));
    let build = manifest["build"][0]["command"]
        .as_array()
        .expect("a build command");
    assert!(build.iter().any(|arg| arg.as_str() == Some("--release")));
}
