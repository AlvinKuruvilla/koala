// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "KoalaBrowser",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(name: "KoalaBrowser", targets: ["KoalaBrowser"])
    ],
    targets: [
        .executableTarget(
            name: "KoalaBrowser",
            path: "Sources/KoalaBrowser"
        )
    ]
)
