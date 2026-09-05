fn main() {
    println!("cargo:rerun-if-changed=native");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut build = cc::Build::new();
        build
            .define("WIN32", None)
            .define("_CRT_SECURE_NO_WARNINGS", None)
            .include("native/soem/soem")
            .include("native/soem/osal")
            .include("native/soem/osal/win32")
            .include("native/soem/oshw/win32")
            .include("native/soem/oshw/win32/wpcap/Include")
            .file("native/bridge.c")
            .file("native/soem/osal/win32/osal.c")
            .file("native/soem/oshw/win32/nicdrv.c")
            .file("native/soem/oshw/win32/oshw.c");
        for file in std::fs::read_dir("native/soem/soem").expect("vendored SOEM") {
            let path = file.expect("SOEM entry").path();
            if path.extension().is_some_and(|e| e == "c") {
                build.file(path);
            }
        }
        build.warnings(false).compile("servo_soem");
        println!("cargo:rustc-link-lib=Ws2_32");
        println!("cargo:rustc-link-lib=Winmm");
    }
    tauri_build::build()
}
