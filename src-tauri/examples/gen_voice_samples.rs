//! gen_voice_samples — render Supertonic voice demos to WAV for the website.
//!
//! Uses the same on-device model as live playback (resolved from ~/.cenno, so
//! the model must be downloaded). Outputs one WAV per voice into the directory
//! given as the first arg (default: ./voice-samples). The site build converts
//! these to web-friendly MP3.
//!
//!     cargo run --example gen_voice_samples -- ../site/public/voices
//!
//! Voices are the Supertonic styles F1–F5 (female) and M1–M5 (male).

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let out_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("voice-samples"));
    std::fs::create_dir_all(&out_dir)?;

    // One ear-friendly line per voice — the agent-style spoken summary cenno
    // would read aloud. Each voice says the same line so they're comparable.
    let line = "Hey — your agent has a question. Tap to answer, or just tell me.";

    // The styles to render. Keep this list aligned with the site's <audio> tags.
    let voices = ["F1", "F2", "F3", "F4", "F5", "M1", "M2", "M3", "M4", "M5"];

    if !cenno_lib::supertonic::assets_present() {
        anyhow::bail!(
            "Supertonic model not found. Download it from cenno Settings → Voice \
             (or set tts.model_path in ~/.cenno/config.json) before generating samples."
        );
    }

    for v in voices {
        let out = out_dir.join(format!("{v}.wav"));
        print!("rendering {v} → {} … ", out.display());
        cenno_lib::supertonic::synth_to_wav(line, v, &out)?;
        println!("ok ({} bytes)", std::fs::metadata(&out)?.len());
    }
    println!("done → {}", out_dir.display());
    Ok(())
}
