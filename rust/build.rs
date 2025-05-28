use std::env;

fn main() {
    // Tell cargo to link against the C standard library
    println!("cargo:rustc-link-lib=c");
    
    // Set up include paths
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let parent_dir = std::path::Path::new(&manifest_dir).parent().unwrap();
    
    println!("cargo:rustc-env=LIBXML2_INCLUDE_DIR={}/include", parent_dir.display());
}