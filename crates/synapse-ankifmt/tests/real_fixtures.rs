//! Any real `.apkg`/`.colpkg` dropped into the repo's `fixtures/` directory is
//! picked up here: it must either parse to a non-empty model, or fail with the
//! known "v18 not yet supported" error. This is how real-world exports join the
//! test corpus without code changes.

use std::path::PathBuf;

use synapse_ankifmt::read_package;

#[test]
fn real_fixtures_parse_or_are_known_unsupported() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let Ok(entries) = std::fs::read_dir(&fixtures) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "apkg" && ext != "colpkg" {
            continue;
        }

        match read_package(&path, None) {
            Ok((model, _)) => {
                assert!(
                    !model.is_empty(),
                    "fixture {path:?} parsed to an empty model"
                );
            }
            Err(error) => {
                let message = error.to_string();
                assert!(
                    message.contains("v18") || message.contains("supported"),
                    "fixture {path:?} failed unexpectedly: {message}"
                );
            }
        }
    }
}
