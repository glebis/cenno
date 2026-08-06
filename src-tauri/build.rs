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

    // Bake an rpath to the system Swift runtime so `cargo test`/`cargo run`
    // binaries can load @rpath/libswift_Concurrency.dylib without a DYLD_* env
    // (which macOS strips). Harmless for the release .app, which finds it via
    // the OS anyway.
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");

    tauri_build::build();
}
