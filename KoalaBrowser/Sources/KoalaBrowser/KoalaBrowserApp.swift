import SwiftUI
import KoalaCore

@main
struct KoalaBrowserApp: App {
    /// Initial URL from command line arguments
    private let initialURL: String?

    init() {
        // Parse command line arguments
        // Usage: koala [path]
        let args = CommandLine.arguments

        var foundURL: String? = nil

        for i in 1..<args.count {
            let arg = args[i]
            if !arg.hasPrefix("-") {
                // Convert relative paths to absolute
                if arg.hasPrefix("/") || arg.hasPrefix("file://") {
                    foundURL = arg
                } else {
                    let cwd = FileManager.default.currentDirectoryPath
                    foundURL = "\(cwd)/\(arg)"
                }
                break
            }
        }

        self.initialURL = foundURL
    }

    var body: some Scene {
        WindowGroup {
            BrowserView(initialURL: initialURL)
        }
        .windowStyle(.automatic)
        .commands {
            CommandGroup(replacing: .newItem) { }
        }
    }
}
