// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "Loopflow",
    platforms: [
        .macOS(.v15)
    ],
    products: [
        .executable(name: "Loopflow", targets: ["Loopflow"])
    ],
    targets: [
        .executableTarget(
            name: "Loopflow",
            path: "Loopflow"
        ),
        .testTarget(
            name: "LoopflowTests",
            dependencies: ["Loopflow"],
            path: "LoopflowTests"
        )
    ]
)
