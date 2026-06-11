use swift_rs::SwiftLinker;

fn main() {
    // Compile + link the CennoVoice Swift package (SpeechTranscriber dictation).
    SwiftLinker::new("13.0")
        .with_package("CennoVoice", "swift")
        .link();
    tauri_build::build();
}
