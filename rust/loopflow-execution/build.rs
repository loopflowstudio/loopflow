use std::env;
fn main() {
    println!("cargo:rerun-if-changed=src/installation/registration.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/installation/registration.m")
            .flag("-fobjc-arc")
            .compile("server_registration");
        println!("cargo:rustc-link-lib=framework=ServiceManagement");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rerun-if-changed=src/journal/os_process/macos.c");
        cc::Build::new()
            .file("src/journal/os_process/macos.c")
            .compile("loopflow_process_info");
    }
}
