import SwiftUI
import WatchKit
import CennoSharedWatch

@main
struct CennoWatchApp: App {
    @StateObject private var relay = CloudKitRelay()
    @WKApplicationDelegateAdaptor private var delegate: WatchAppDelegate

    var body: some Scene {
        WindowGroup {
            PromptQueueView()
                .environmentObject(relay)
                .task {
                    #if DEBUG
                    if let demo = WatchDemo.requestedKind {
                        relay.pendingPrompts = [WatchDemo.prompt(kind: demo)]
                        return
                    }
                    #endif
                    await relay.start()
                }
                .onReceive(NotificationCenter.default.publisher(for: .cennoRemotePush)) { _ in
                    Task { await relay.handleRemoteNotification() }
                }
        }
    }
}

#if DEBUG
/// Headless verification: launch with `-cennoDemo <kind>` to seed one prompt
/// (no CloudKit) so the native Watch rendering can be screenshotted.
enum WatchDemo {
    static var requestedKind: String? {
        let args = ProcessInfo.processInfo.arguments
        guard let i = args.firstIndex(of: "-cennoDemo"), i + 1 < args.count else { return nil }
        return args[i + 1]
    }
    static func prompt(kind: String) -> PromptRecord {
        let json = """
        {"title":"How focused are you?","body_md":"Be honest.",
         "input":{"kind":"\(kind)"},"choices":["Calm","Tense","Wired"],"flow":"question"}
        """
        let payload = try! JSONDecoder().decode(PromptPayload.self, from: Data(json.utf8))
        return PromptRecord(id: "watch-demo-\(kind)", payload: payload, deviceHint: .watch,
                            state: .pending, answer: nil,
                            createdAt: Date(), expiresAt: Date().addingTimeInterval(300))
    }
}
#endif

// MARK: - App delegate for background push

final class WatchAppDelegate: NSObject, WKApplicationDelegate {
    func didReceiveRemoteNotification(
        _ userInfo: [AnyHashable: Any],
        fetchCompletionHandler completionHandler: @escaping (WKBackgroundFetchResult) -> Void
    ) {
        NotificationCenter.default.post(name: .cennoRemotePush, object: nil)
        completionHandler(.newData)
    }
}

extension Notification.Name {
    static let cennoRemotePush = Notification.Name("app.cenno.remotePush")
}
