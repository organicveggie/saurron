fn main() {
    let version =
        std::env::var("SAURRON_BUILD_VERSION").unwrap_or_else(|_| "v0.0.0-unknown".to_string());
    println!("cargo:rustc-env=SAURRON_VERSION={version}");
    println!("cargo:rerun-if-env-changed=SAURRON_BUILD_VERSION");
}
