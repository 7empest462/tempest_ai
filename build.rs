fn main() {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    println!("cargo:rustc-env=BUILD_TIME={}", now);
    println!("cargo:rerun-if-changed=build.rs");
}
