import SwiftUI
import WatchKit
import CennoShared

@main
struct CennoWatchApp: App {
    @StateObject private var relay = CloudKitRelay()
    @WKApplicationDelegateAdaptor private var delegate: WatchAppDelegate

    var body: some Scene {
        WindowGroup {
            PromptQueueView()
                .environmentObject(relay)
                .task { await relay.start() }
                .onReceive(NotificationCenter.default.publisher(for: .cennoRemotePush)) { _ in
                    Task { await relay.handleRemoteNotification() }
                }
        }
    }
}

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
