// Quick test to verify FFI integration
import Foundation
import KoalaCore

func testFFI() {
    let html = "<html><body><h1>Test</h1><p>Hello World</p></body></html>"

    print("Testing Rust FFI integration...")
    print("Input HTML: \(html)")

    if let json = KoalaParser.parseHTML(html) {
        print("\nJSON from Rust:")
        print(json)

        if let dom = KoalaParser.parse(html) {
            print("\nDecoded DOMNode:")
            print("  type: \(dom.type)")
            print("  children: \(dom.childNodes.count)")
            printNode(dom, indent: 2)
        } else {
            print("ERROR: Failed to decode JSON to DOMNode")
        }
    } else {
        print("ERROR: KoalaParser.parseHTML returned nil")
    }
}

func printNode(_ node: DOMNode, indent: Int) {
    let spaces = String(repeating: " ", count: indent)
    print("\(spaces)- type: \(node.type), tag: \(node.tagName ?? "nil"), content: \(node.content?.prefix(30) ?? "nil")")
    for child in node.childNodes {
        printNode(child, indent: indent + 2)
    }
}

// Run test when loaded
// testFFI()
