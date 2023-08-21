use anyhow::*;
use fs_extra::{copy_items, dir::CopyOptions};
use std::env;

fn main() -> Result<()> {
    // tell cargo to rerun this script if something in /public/ changes.
    println!("cargo:rerun-if-changed=public/*");

    let out_dir = env::var("OUT_DIR")?;
    let mut copy_opts = CopyOptions::new();
    copy_opts.overwrite = true;
    let mut paths_to_copy = Vec::new();
    paths_to_copy.push("public/");
    copy_items(&paths_to_copy, out_dir, &copy_opts)?;

    Ok(())
}
