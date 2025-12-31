import SwiftUI
import KoalaCore

@main
struct KoalaBrowserApp: App {
    init() {
        // Test FFI on startup
        print("=== Testing Rust FFI ===")
        let testHtml = "<html><body><h1>Test</h1></body></html>"
        if let json = KoalaParser.parseHTML(testHtml) {
            print("FFI works! JSON: \(json.prefix(200))...")
            if let dom = KoalaParser.parse(testHtml) {
                print("Decode works! type=\(dom.type) children=\(dom.childNodes.count)")
            } else {
                print("ERROR: JSON decode failed")
            }
        } else {
            print("ERROR: FFI returned nil")
        }
        print("========================")
    }

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
