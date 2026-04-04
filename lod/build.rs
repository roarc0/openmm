// Vendored from greg-kennedy/libsmacker v1.2.0
// https://github.com/greg-kennedy/libsmacker
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    cc::Build::new()
        .file("vendor/libsmacker/smacker.c")
        .include("vendor/libsmacker")
        .warnings(false)
        .compile("smacker");
    println!("cargo:rerun-if-changed=vendor/libsmacker/smacker.c");
    println!("cargo:rerun-if-changed=vendor/libsmacker/smacker.h");
    println!("cargo:rerun-if-changed=vendor/libsmacker/smk_malloc.h");
}
