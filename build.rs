use std::{
    env,
    fs::{self, DirBuilder},
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

// copies assets to the target directory, recursively

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    let executable_path = locate_target_dir(&out_dir)
        .expect("failed to find target dir")
        .join(env::var("PROFILE").unwrap());

    copy(
        &manifest_dir.join("assets"),
        &executable_path.join("assets"),
    )
}

// find the target directory
fn locate_target_dir(mut target_dir: &Path) -> Option<&Path> {
    loop {
        if target_dir.ends_with("target") {
            return Some(target_dir);
        }

        target_dir = match target_dir.parent() {
            Some(path) => path,
            None => break,
        }
    }

    None
}

fn copy(from: &Path, to: &Path) {
    let from_path: PathBuf = from.into();
    let to_path: PathBuf = to.into();

    for entry in WalkDir::new(from_path.clone()) {
        let entry = entry.unwrap();

        if let Ok(rel_path) = entry.path().strip_prefix(&from_path) {
            let target_path = to_path.join(rel_path);

            if entry.file_type().is_dir() {
                DirBuilder::new()
                    .recursive(true)
                    .create(target_path).expect("failed to create target dir");
            } else {
                fs::copy(entry.path(), &target_path).expect("failed to copy");
            }
        }
    }
}