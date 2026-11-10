use swift_rs::SwiftLinker;

fn main() {
    // Compile + link the CennoVoice Swift package (SpeechTranscriber dictation).
    SwiftLinker::new("13.0")
        .with_package("CennoVoice", "swift")
        .link();

    // Compile + link the CennoRelay Swift package (CloudKit relay writer).
    SwiftLinker::new("13.0")
        .with_package("CennoRelay", "swift")
        .link();

    // Compile + link the Accessibility-only focused screen-context reader.
    SwiftLinker::new("13.0")
        .with_package("CennoScreenContext", "swift")
        .link();

    // swift-rs 1.0.7 points the linker at `<pkg>/arm64-apple-macosx/release`,
    // but SwiftPM's new build system (Swift 6.4+) writes the static libs to
    // `<pkg>/out/Products/Release` instead. Add that path too so either
    // toolchain links (a stale/missing search path is harmless).
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR set by cargo");
    let profile = if std::env::var("PROFILE").as_deref() == Ok("release") {
        "Release"
    } else {
        "Debug"
    };
    for pkg in ["CennoVoice", "CennoRelay", "CennoScreenContext"] {
        println!("cargo:rustc-link-search=native={out_dir}/swift-rs/{pkg}/out/Products/{profile}");
    }

    // Bake an rpath to the system Swift runtime so `cargo test`/`cargo run`
    // binaries can load @rpath/libswift_Concurrency.dylib without a DYLD_* env
    // (which macOS strips). Harmless for the release .app, which finds it via
    // the OS anyway.
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");

    tauri_build::build();
}
