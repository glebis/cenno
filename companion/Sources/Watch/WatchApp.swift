import SwiftUI
import CennoShared

@main
struct CennoWatchApp: App {
    @StateObject private var relay = CloudKitRelay()

    var body: some Scene {
        WindowGroup {
            PromptQueueView()
                .environmentObject(relay)
                .task { await relay.start() }
        }
    }
}
