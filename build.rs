use std::{
    env,
    fs,
    path::Path,
    process::Command,
};

fn main() {
    // ============================
    // Metadata
    // ============================

    let build_time = String::from_utf8(
        Command::new("date")
            .arg("+%Y-%m-%d %H:%M:%S")
            .output()
            .expect("failed to execute date")
            .stdout,
    )
    .unwrap()
    .trim()
    .to_owned();

    let rust_version = String::from_utf8(
        Command::new("rustc")
            .arg("--version")
            .output()
            .expect("failed to execute rustc")
            .stdout,
    )
    .unwrap()
    .trim()
    .to_owned();

    let git_hash = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()
            .expect("failed to execute git")
            .stdout,
    )
    .unwrap()
    .trim()
    .to_owned();

    println!("cargo:rustc-env=BUILD_TIME={build_time}");
    println!("cargo:rustc-env=RUSTC_VERSION={rust_version}");
    println!("cargo:rustc-env=GIT_HASH={git_hash}");

    // ============================
    // Generate atos_intro.txt
    // ============================

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let mut intro =
        fs::read_to_string(format!("{manifest_dir}/atos_intro.txt"))
            .expect("failed to read atos_intro.txt");

    intro = intro.replace("{name}", env!("CARGO_PKG_NAME"));
    intro = intro.replace("{version}", env!("CARGO_PKG_VERSION"));
    intro = intro.replace("{build}", &build_time);
    intro = intro.replace("{rust}", &rust_version);
    intro = intro.replace("{git}", &git_hash);

    let out_path = Path::new(&env::var("OUT_DIR").unwrap())
        .join("atos_intro_generated.txt");

    fs::write(out_path, intro).unwrap();

    // ============================
    // Rebuild rules
    // ============================

    println!("cargo:rerun-if-changed=atos_intro.txt");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=entry.S");
    println!("cargo:rerun-if-changed=src/kernel/exceptions.s");

    // ============================
    // Assemble startup code
    // ============================

    cc::Build::new()
        .file("entry.S")
        .file("src/kernel/exceptions.s")
        .compiler("aarch64-linux-gnu-gcc")
        .flag("-c")
        .compile("entry");
}