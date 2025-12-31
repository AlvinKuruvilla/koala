import SwiftUI

@main
struct KoalaBrowserApp: App {
    var body: some Scene {
        WindowGroup {
            BrowserView()
        }
        .windowStyle(.automatic)
        .commands {
            CommandGroup(replacing: .newItem) { }
        }
    }
}
