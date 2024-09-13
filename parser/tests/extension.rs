use parser::extension::Extensions;
use std::{fs::File, path::PathBuf};

#[test]
fn load_usfm_ext() {
    let reader = File::open(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("usfm.ext"),
    )
    .expect("usfm.ext");
    let res = Extensions::from_reader(reader);
    let markers;
    if let Err(err) = res {
        eprintln!("loading usfm.ext failed:\n{err}\n");
        panic!("{err:?}");
    } else {
        markers = res.unwrap();
    }
    assert_eq!(markers.len(), 302);
}
