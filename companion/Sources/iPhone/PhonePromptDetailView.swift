import SwiftUI
import CennoShared

/// The detail screen now renders every prompt through the A2UI runtime
/// (A2UIPromptView), matching the tauri macOS panel's renderer.
struct PhonePromptDetailView: View {
    let prompt: PromptRecord
    var body: some View { A2UIPromptView(prompt: prompt) }
}
