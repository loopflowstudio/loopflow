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
    targets: [
        .executableTarget(
            name: "Maestro",
            path: "Maestro",
            exclude: ["Info.plist"]
        ),
        .testTarget(
            name: "MaestroTests",
            dependencies: ["Maestro"],
            path: "MaestroTests"
        )
    ]
)
