// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "SnipSprig",
    platforms: [.macOS(.v13)],
    products: [.executable(name: "SnipSprig", targets: ["SnipSprig"])],
    targets: [
        .target(name: "SnipSprigKit"),
        .executableTarget(name: "SnipSprig", dependencies: ["SnipSprigKit"]),
        .testTarget(name: "SnipSprigKitTests", dependencies: ["SnipSprigKit"])
    ]
)
