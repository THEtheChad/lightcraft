//! Reproduces originals::tests::a_hard_link_whose_identity_cannot_be_read_is_refused step by step,
//! mirroring lightcraft_catalog::safe_file::compare_files(strict = true) on Windows.
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use winapi_util::{file::information, Handle};

fn ids(p: &Path) -> std::io::Result<(u64, u64)> {
    let h = Handle::from_path_any(p)?;
    let i = information(&h)?;
    Ok((i.volume_serial_number(), i.file_index()))
}

fn strict_same(a: &Path, b: &Path) -> String {
    let ma = std::fs::metadata(a);
    let mb = std::fs::metadata(b);
    let (ma, mb) = match (ma, mb) {
        (Ok(x), Ok(y)) => (x, y),
        (x, y) => return format!("metadata err: a={:?} b={:?}", x.err(), y.err()),
    };
    if ma.len() != mb.len() {
        return "Ok(false) (lengths differ)".into();
    }
    match (ids(a), ids(b)) {
        (Ok(ia), Ok(ib)) => format!("file_ids Ok: a={ia:?} b={ib:?} same={}", ia == ib && ia.1 != 0),
        (x, y) => format!("file_ids Err (strict => 'cannot tell'): a={:?} b={:?}", x.err(), y.err()),
    }
}

fn run(base: PathBuf) {
    println!("\n===== base: {}", base.display());
    let d = base.join(format!("lc-repro-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    if let Err(e) = std::fs::create_dir_all(&d) {
        println!("cannot create {}: {e}", d.display());
        return;
    }
    let original = d.join("IMG_1.jpg");
    std::fs::write(&original, b"original").unwrap();
    let linked = d.join("shot.png");
    if let Err(e) = std::fs::hard_link(&original, &linked) {
        println!("hard_link failed: {e}");
        return;
    }
    println!("before hold: ids(original)={:?} ids(linked)={:?}", ids(&original), ids(&linked));
    let held = std::fs::OpenOptions::new().read(true).share_mode(0).open(&original).unwrap();
    println!("held read + share_mode(0) on original");
    println!("  metadata(original)    = {:?}", std::fs::metadata(&original).map(|m| m.len()));
    println!("  metadata(linked)      = {:?}", std::fs::metadata(&linked).map(|m| m.len()));
    println!("  canonicalize(linked)  = {:?}", std::fs::canonicalize(&linked));
    println!("  ids(original)         = {:?}", ids(&original));
    println!("  ids(linked)           = {:?}", ids(&linked));
    println!("  File::open(linked)    = {:?}", std::fs::File::open(&linked).map(|_| "ok"));
    println!("  File::open(original)  = {:?}", std::fs::File::open(&original).map(|_| "ok"));
    println!("  strict_same(orig,lnk) = {}", strict_same(&original, &linked));
    drop(held);
    println!("after drop: strict_same = {}", strict_same(&original, &linked));
    let _ = std::fs::remove_dir_all(&d);
}

fn main() {
    let mut bases = vec![std::env::temp_dir()];
    for var in ["RUNNER_TEMP", "GITHUB_WORKSPACE"] {
        if let Ok(v) = std::env::var(var) {
            bases.push(PathBuf::from(v));
        }
    }
    for b in bases {
        run(b);
    }
}
