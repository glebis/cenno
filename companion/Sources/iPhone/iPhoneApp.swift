import SwiftUI
import CennoShared

@main
struct CennoiPhoneApp: App {
    @StateObject private var relay = CloudKitRelay()
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            PhonePromptQueueView()
                .environmentObject(relay)
                .task { await relay.start() }
        }
        .onChange(of: phase) { _, new in
            if new == .active {
                Task { await relay.handleRemoteNotification() }
            }
        }
    }
}
