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

    /// `-cennoReplyOpen` — auto-open the free-text reply sheet on appear, for
    /// screenshotting the composer.
    static var replyOpen: Bool {
        ProcessInfo.processInfo.arguments.contains("-cennoReplyOpen")
    }

    /// `-cennoA2UI <component>` — render a single a2ui catalog component
    /// (slider|datetime|dots|image|scorematrix) via the passthrough path, to
    /// screenshot controls the simple input-kind shortcuts can't reach.
    static var a2uiComponent: String? {
        let args = ProcessInfo.processInfo.arguments
        guard let i = args.firstIndex(of: "-cennoA2UI"), i + 1 < args.count else { return nil }
        return args[i + 1]
    }

    /// Build a passthrough prompt wrapping a single control component.
    static func a2uiPrompt(component: String) -> PromptRecord {
        let (title, ctl): (String, String)
        switch component {
        case "slider":
            title = "How loud should alerts be?"
            ctl = #"{"id":"ctl","component":"Slider","label":"Volume","min":0,"max":10,"value":4,"minLabel":"quiet","maxLabel":"loud","selectAction":{"event":{"name":"submit-slider","context":{"value":{"path":"/v"},"via":"choice"}}}}"#
        case "datetime":
            title = "When should I follow up?"
            ctl = #"{"id":"ctl","component":"DateTimeInput","label":"Follow-up date","enableDate":true,"enableTime":false,"value":"2026-06-20","submitAction":{"event":{"name":"submit-date","context":{"value":{"path":"/v"},"via":"text"}}}}"#
        case "dots":
            title = "Step 2 of 5"
            ctl = #"{"id":"ctl","component":"Dots","step":2,"total":5}"#
        case "image":
            title = "Does this crop look right?"
            // Inline SVG data URI — a teal square with a check, no network needed.
            ctl = #"{"id":"ctl","component":"Image","fit":"contain","description":"preview","url":"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='240' height='160'><rect width='240' height='160' rx='16' fill='%230E7C6B'/><path d='M90 80 l24 24 l44 -52' stroke='white' stroke-width='12' fill='none' stroke-linecap='round' stroke-linejoin='round'/></svg>"}"#
        case "scorematrix":
            title = "Score the bottleneck"
            ctl = #"{"id":"ctl","component":"ScoreMatrix","legend":"0 none · 3 high","steps":["Intake review","Triage","Build"],"submitAction":{"event":{"name":"submit-matrix","context":{"value":{"path":"/v"},"via":"text"}}}}"#
        default:
            title = "Unknown component"
            ctl = #"{"id":"ctl","component":"Text","text":"unknown: \#(component)"}"#
        }
        let json = """
        {"title":"\(title)","input":{"kind":"none"},
         "a2ui":[
           {"version":"v0.9","createSurface":{"surfaceId":"main","catalogId":"cenno:catalog/v1"}},
           {"version":"v0.9","updateComponents":{"surfaceId":"main","components":[
             {"id":"root","component":"Column","children":["col"]},
             {"id":"col","component":"Column","children":["title","ctl"]},
             {"id":"title","component":"Text","variant":"h2","text":"\(title)"},
             \(ctl)
           ]}},
           {"version":"v0.9","updateDataModel":{"surfaceId":"main","path":"/","value":{"v":null}}}
         ]}
        """
        let payload = try! JSONDecoder().decode(PromptPayload.self, from: Data(json.utf8))
        return PromptRecord(id: "demo-a2ui-\(component)", payload: payload, deviceHint: .any,
                            state: .pending, answer: nil,
                            createdAt: Date(), expiresAt: Date().addingTimeInterval(600))
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
