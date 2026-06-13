#if DEBUG
import SwiftUI
import CennoShared

/// DEBUG-only verification harness. Launch the app with
/// `-cennoDemo <kind>` (text|voice_text|choice|scale|confirm|none|markdown)
/// to render a synthetic prompt straight to the screen for screenshots —
/// no CloudKit needed. Used to verify the A2UI renderer in the simulator.
enum DemoHarness {
    static var requestedKind: String? {
        let args = ProcessInfo.processInfo.arguments
        guard let i = args.firstIndex(of: "-cennoDemo"), i + 1 < args.count else { return nil }
        return args[i + 1]
    }

    static func prompt(kind: String) -> PromptRecord {
        // "markdown" exercises rich body formatting on a plain text prompt.
        let inputKind = (kind == "markdown") ? "text" : kind
        let json = """
        {"title":"How **focused** are you?",
         "body_md":"_Be honest._ Tap below. Lists work too:\\n\\n- one\\n- two\\n\\nAnd `code`.",
         "input":{"kind":"\(inputKind)"},
         "choices":["Calm","Tense","Wired"]}
        """
        let payload = try! JSONDecoder().decode(PromptPayload.self, from: Data(json.utf8))
        return PromptRecord(id: "demo-\(kind)", payload: payload, deviceHint: .iphone,
                            state: .pending, answer: nil,
                            createdAt: Date(), expiresAt: Date().addingTimeInterval(300))
    }
}

struct DemoRootView: View {
    let kind: String
    @EnvironmentObject var relay: CloudKitRelay
    var body: some View {
        NavigationStack {
            A2UIPromptView(prompt: DemoHarness.prompt(kind: kind))
                .navigationTitle(kind)
        }
    }
}
#endif
