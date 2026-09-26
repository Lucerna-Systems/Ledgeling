fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("native/pet_ui.m")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .flag("-Wno-deprecated-declarations")
            .compile("perch_pet_ui");
        println!("cargo:rustc-link-lib=framework=Cocoa");
        println!("cargo:rerun-if-changed=native/pet_ui.m");
    }
    // An explicit rerun-if-changed replaces Cargo's watch-everything default, so
    // the icons baked in by generate_context! and include_image! must be named.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
