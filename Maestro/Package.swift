// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "Maestro",
    platforms: [
        .macOS(.v15)
    ],
    products: [
        .executable(name: "Maestro", targets: ["Maestro"])
    ],
    dependencies: [
        .package(url: "https://github.com/migueldeicaza/SwiftTerm", from: "1.2.0")
    ],
    targets: [
        .executableTarget(
            name: "Maestro",
            dependencies: ["SwiftTerm"],
            path: "Maestro",
            exclude: ["Info.plist", "Maestro.sdef"]
        ),
        .testTarget(
            name: "MaestroTests",
            dependencies: ["Maestro"],
            path: "MaestroTests"
        )
    ]
)
