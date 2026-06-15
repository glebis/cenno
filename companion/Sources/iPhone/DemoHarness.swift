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

    /// True when launched with `-cennoQueueDemo` — seeds the REAL queue so the
    /// queue → tap row → detail navigation path can be exercised (the per-kind
    /// `-cennoDemo` mode bypasses the queue).
    static var queueDemo: Bool {
        ProcessInfo.processInfo.arguments.contains("-cennoQueueDemo")
    }

    static func queuePrompts() -> [PromptRecord] {
        ["choice", "scale", "confirm", "text"].map { prompt(kind: $0) }
    }

    /// `-cennoSecondScreen idle|prompt` — render the ambient second screen for
    /// screenshots. `idle` shows the clock; `prompt` seeds a mirror-routed prompt
    /// that surfaces immediately. No CloudKit (polling disabled).
    static var secondScreen: String? {
        let args = ProcessInfo.processInfo.arguments
        guard let i = args.firstIndex(of: "-cennoSecondScreen"), i + 1 < args.count else { return nil }
        return args[i + 1]
    }

    /// A prompt explicitly routed to mirror on this device — demos the
    /// "surfaced on the second screen" state.
    static func secondScreenPrompt() -> PromptRecord {
        let json = """
        {"title":"Ship the v0.3 build to TestFlight?",
         "body_md":"Signing + notarization passed. This pushes to external testers.",
         "input":{"kind":"confirm"}}
        """
        let payload = try! JSONDecoder().decode(PromptPayload.self, from: Data(json.utf8))
        let cls = DeviceClass.current
        return PromptRecord(id: "demo-second-screen", payload: payload, deviceHint: .any,
                            state: .pending, answer: nil,
                            createdAt: Date(), expiresAt: Date().addingTimeInterval(600),
                            targets: RoutingTargets([cls: .mirror]), graceS: 20)
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
    @State private var answered: PromptAnswer?
    var body: some View {
        NavigationStack {
            A2UIPromptView(prompt: DemoHarness.prompt(kind: kind)) { answer in
                withAnimation { answered = answer }
            }
            .navigationTitle(kind)
            .overlay(alignment: .bottom) {
                if let answered {
                    Text("✓ answered: \(answered.answer)  (via \(answered.via))")
                        .font(.callout.bold()).foregroundStyle(.white)
                        .padding().frame(maxWidth: .infinity)
                        .background(.black.opacity(0.5))
                        .transition(.move(edge: .bottom))
                }
            }
        }
    }
}
#endif
