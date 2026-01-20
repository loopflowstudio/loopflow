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
        .package(url: "https://github.com/nalexn/ViewInspector.git", from: "0.10.0")
    ],
    targets: [
        .executableTarget(
            name: "Maestro",
            dependencies: [],
            path: "Maestro",
            exclude: ["Info.plist", "Maestro.sdef"]
        ),
        .testTarget(
            name: "MaestroTests",
            dependencies: ["Maestro", "ViewInspector"],
            path: "MaestroTests"
        )
    ]
)
