fn main() {
    // Tell cargo where to find the compiled SERAPH cdylib.
    //
    // At build time, the consumer sets SERAPH_LIB_DIR to point at the
    // directory containing seraph.dll / libseraph.so / libseraph.dylib.
    //
    // Example:
    //   SERAPH_LIB_DIR=/path/to/seraph/lib cargo build
    if let Ok(lib_dir) = std::env::var("SERAPH_LIB_DIR") {
        println!("cargo:rustc-link-search=native={lib_dir}");

        // On Windows, cargo produces seraph.dll.lib but the linker expects
        // seraph.lib. Copy it if needed.
        let dll_lib = std::path::Path::new(&lib_dir).join("seraph.dll.lib");
        let import_lib = std::path::Path::new(&lib_dir).join("seraph.lib");
        if dll_lib.exists() && !import_lib.exists() {
            let _ = std::fs::copy(&dll_lib, &import_lib);
        }
    }
    println!("cargo:rustc-link-lib=dylib=seraph");
}
