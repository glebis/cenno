import SwiftUI
import UIKit
import CennoShared
import UserNotifications

@main
struct CennoiPhoneApp: App {
    @UIApplicationDelegateAdaptor private var delegate: PhoneAppDelegate
    @StateObject private var relay = CloudKitRelay()
    @StateObject private var secondScreen = SecondScreenSettings()

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
        if let mode = DemoHarness.secondScreen {
            // Ambient second-screen screenshots (no CloudKit; polling disabled).
            SecondScreenView(settings: secondScreen, poll: false)
                .task {
                    relay.pendingPrompts = (mode == "prompt") ? [DemoHarness.secondScreenPrompt()] : []
                }
        } else if let comp = DemoHarness.a2uiComponent {
            // Single a2ui catalog component via the passthrough path.
            NavigationStack { A2UIPromptView(prompt: DemoHarness.a2uiPrompt(component: comp)) }
        } else if let kind = DemoHarness.requestedKind {
            DemoRootView(kind: kind)   // headless renderer verification; skips CloudKit
        } else if DemoHarness.queueDemo {
            // Exercise the real queue → tap → detail path with seeded prompts.
            PhonePromptQueueView(secondScreen: secondScreen)
                .task { relay.pendingPrompts = DemoHarness.queuePrompts() }
        } else {
            appRoot.task { await relay.start() }
        }
        #else
        appRoot.task { await relay.start() }
        #endif
    }

    /// When the user has flipped this device into second-screen mode, the
    /// ambient display IS the app; otherwise the normal pull-to-refresh queue.
    @ViewBuilder
    private var appRoot: some View {
        if secondScreen.enabled {
            SecondScreenView(settings: secondScreen)
        } else {
            PhonePromptQueueView(secondScreen: secondScreen)
        }
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
