import SwiftUI
import UIKit
import CennoShared
import UserNotifications

@main
struct CennoiPhoneApp: App {
    @UIApplicationDelegateAdaptor private var delegate: PhoneAppDelegate
    @StateObject private var relay = CloudKitRelay()

    var body: some Scene {
        WindowGroup {
            rootView
                .environmentObject(relay)
                .onReceive(NotificationCenter.default.publisher(for: .cennoRemotePush)) { _ in
                    Task { await relay.handleRemoteNotification() }
                }
        }
    }

    @ViewBuilder
    private var rootView: some View {
        #if DEBUG
        if let kind = DemoHarness.requestedKind {
            DemoRootView(kind: kind)   // headless renderer verification; skips CloudKit
        } else if DemoHarness.queueDemo {
            // Exercise the real queue → tap → detail path with seeded prompts.
            PhonePromptQueueView().task { relay.pendingPrompts = DemoHarness.queuePrompts() }
        } else {
            PhonePromptQueueView().task { await relay.start() }
        }
        #else
        PhonePromptQueueView().task { await relay.start() }
        #endif
    }
}

// MARK: - App delegate for push registration + silent push delivery

final class PhoneAppDelegate: NSObject, UIApplicationDelegate {
    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        // Register for remote notifications (no user permission needed for silent pushes).
        application.registerForRemoteNotifications()
        return true
    }

    func application(
        _ application: UIApplication,
        didReceiveRemoteNotification userInfo: [AnyHashable: Any],
        fetchCompletionHandler completionHandler: @escaping (UIBackgroundFetchResult) -> Void
    ) {
        NotificationCenter.default.post(name: .cennoRemotePush, object: nil)
        completionHandler(.newData)
    }
}

extension Notification.Name {
    static let cennoRemotePush = Notification.Name("app.cenno.remotePush")
}
