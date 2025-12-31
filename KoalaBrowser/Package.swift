// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "KoalaBrowser",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(name: "KoalaBrowser", targets: ["KoalaBrowser"]),
        .executable(name: "koala-screenshot", targets: ["KoalaScreenshot"])
    ],
    targets: [
        // System library target that wraps the Rust FFI
        .systemLibrary(
            name: "CKoala",
            path: "Sources/KoalaCore/include"
        ),
        // Swift wrapper for the Rust library
        .target(
            name: "KoalaCore",
            dependencies: ["CKoala"],
            path: "Sources/KoalaCore",
            exclude: ["include"],
            linkerSettings: [
                .unsafeFlags([
                    "-L", "../target/release",
                    "-lkoala"
                ])
            ]
        ),
        // Main browser application
        .executableTarget(
            name: "KoalaBrowser",
            dependencies: ["KoalaCore"],
            path: "Sources/KoalaBrowser"
        ),
        // Headless screenshot tool
        .executableTarget(
            name: "KoalaScreenshot",
            dependencies: ["KoalaCore"],
            path: "Sources/KoalaScreenshot"
        )
    ]
)
