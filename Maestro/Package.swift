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
    dependencies: [],
    targets: [
        .executableTarget(
            name: "Maestro",
            dependencies: [],
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
