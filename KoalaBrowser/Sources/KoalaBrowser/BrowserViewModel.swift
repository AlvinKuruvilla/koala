import SwiftUI
import Foundation
import KoalaCore

@MainActor
class BrowserViewModel: ObservableObject {
    @Published var urlInput: String = ""
    @Published var currentURL: String = ""
    @Published var document: DOMNode?
    @Published var error: String?
    @Published var isLoading: Bool = false
    @Published var canGoBack: Bool = false
    @Published var canGoForward: Bool = false
    @Published var showDebugPanel: Bool = false
    @Published var rawHTML: String = ""
    @Published var rawJSON: String = ""

    var securityIcon: String {
        if currentURL.hasPrefix("file://") || !currentURL.contains("://") {
            return "doc"
        } else if currentURL.hasPrefix("https://") {
            return "lock.fill"
        } else {
            return "globe"
        }
    }

    var securityColor: Color {
        if currentURL.hasPrefix("file://") || !currentURL.contains("://") {
            return .secondary
        } else if currentURL.hasPrefix("https://") {
            return .green
        } else {
            return .secondary
        }
    }

    func loadURL() {
        let url = urlInput.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !url.isEmpty else { return }

        isLoading = true
        error = nil
        document = nil
        currentURL = url

        // Resolve path
        let path: String
        if url.hasPrefix("file://") {
            path = String(url.dropFirst(7))
        } else {
            path = url
        }

        // Try to load the file
        do {
            let html = try String(contentsOfFile: path, encoding: .utf8)
            rawHTML = html
            print("[DEBUG] Loaded HTML (\(html.count) chars)")

            // Use the Rust parser via FFI
            if let json = KoalaParser.parseHTML(html) {
                rawJSON = json
                print("[DEBUG] Got JSON from Rust: \(json.prefix(500))...")
                if let dom = KoalaParser.parse(html) {
                    document = dom
                    print("[INFO] Successfully parsed: \(path)")
                    print("[DEBUG] Document type: \(dom.type), children: \(dom.childNodes.count)")
                } else {
                    self.error = "Failed to decode JSON to DOMNode"
                    print("[ERROR] JSON decode failed")
                }
            } else {
                self.error = "Failed to parse HTML"
                print("[ERROR] Rust parser returned nil for \(path)")
            }
        } catch {
            self.error = "Failed to load: \(error.localizedDescription)"
            print("[ERROR] Failed to load \(path): \(error)")
        }

        isLoading = false
    }

    func goBack() {
        // TODO: Implement history
    }

    func goForward() {
        // TODO: Implement history
    }

    func refresh() {
        loadURL()
    }
}
