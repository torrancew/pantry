use std::{
    env,
    io::{self, Write},
    path::PathBuf,
    sync::Arc,
};

use macro_rules_attribute::apply;
use smol::process::Command;
use smol_macros::{main, Executor};

#[apply(main)]
async fn main(_ex: Arc<Executor<'_>>) -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=sass/styles.sass");
    println!("cargo::rerun-if-changed=ts/screen-wake.ts");
    println!("cargo::rerun-if-changed=ts/table-sort.ts");
    println!("cargo::rerun-if-changed=ts/theme-switcher.ts");

    let asset_dir = env::var("OUT_DIR").map(PathBuf::from)?.join("assets");
    smol::fs::create_dir_all(&asset_dir).await?;

    let sassc = Command::new("sassc")
        .arg("--sass")
        .arg("sass/styles.sass")
        .arg(asset_dir.join("styles.css"))
        .output()
        .await?;

    if !sassc.status.success() {
        io::stdout().write_all(&sassc.stdout)?;
        io::stderr().write_all(&sassc.stderr)?;
        std::process::exit(sassc.status.code().unwrap())
    }

    let tsc = Command::new("tsc")
        .args([
            "--outDir",
            &asset_dir.to_string_lossy(),
            "--module",
            "es2022",
            "--target",
            "es2022",
        ])
        .arg("./ts/screen-wake.ts")
        .arg("./ts/table-sort.ts")
        .arg("./ts/theme-switcher.ts")
        .output()
        .await?;

    if !tsc.status.success() {
        io::stdout().write_all(&tsc.stdout)?;
        io::stderr().write_all(&tsc.stderr)?;
        std::process::exit(tsc.status.code().unwrap())
    }

    Ok(())
}
