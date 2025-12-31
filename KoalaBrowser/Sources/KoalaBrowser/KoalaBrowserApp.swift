import SwiftUI
import KoalaCore

@main
struct KoalaBrowserApp: App {
    init() {
        // Test FFI on startup - use FileHandle for stderr to ensure output is visible
        let stderr = FileHandle.standardError
        func log(_ msg: String) {
            if let data = (msg + "\n").data(using: .utf8) {
                stderr.write(data)
            }
        }

        log("=== Testing Rust FFI ===")
        let testHtml = "<html><body><h1>Test</h1></body></html>"
        if let json = KoalaParser.parseHTML(testHtml) {
            log("FFI works! JSON: \(String(json.prefix(300)))...")
            if let dom = KoalaParser.parse(testHtml) {
                log("Decode works! type=\(dom.type) children=\(dom.childNodes.count)")
                // Walk the tree
                func walk(_ node: DOMNode, depth: Int) {
                    let indent = String(repeating: "  ", count: depth)
                    log("\(indent)- \(node.type) tag=\(node.tagName ?? "nil") content=\(node.content?.prefix(20) ?? "nil")")
                    for child in node.childNodes {
                        walk(child, depth: depth + 1)
                    }
                }
                walk(dom, depth: 0)
            } else {
                log("ERROR: JSON decode failed")
            }
        } else {
            log("ERROR: FFI returned nil")
        }
        log("========================")
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
