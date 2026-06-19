//! Generate the synthetic fixture corpus into the repo's `fixtures/` dir.
//! Run with: `cargo run -p synapse-ankifmt --example gen_fixtures`.

use std::path::PathBuf;

use synapse_ankifmt::testkit;

fn main() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    std::fs::create_dir_all(&out).expect("create fixtures dir");

    let v11 = out.join("sample-v11.apkg");
    testkit::write_sample_v11_apkg(&v11).expect("write sample v11 apkg");
    println!("wrote {}", v11.display());

    let v18 = out.join("sample-v18.apkg");
    testkit::write_sample_v18_apkg(&v18).expect("write sample v18 apkg");
    println!("wrote {}", v18.display());
}
