// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "LoopflowSwift",
    platforms: [
        .macOS(.v15)
    ],
    products: [
        .library(name: "LoopflowCore", targets: ["LoopflowCore"]),
        .executable(name: "Concerto", targets: ["Concerto"]),
        .executable(name: "Symphonia", targets: ["Symphonia"]),
    ],
    dependencies: [
        .package(url: "https://github.com/nalexn/ViewInspector.git", from: "0.10.0")
    ],
    targets: [
        .target(
            name: "LoopflowCore",
            path: "LoopflowCore"
        ),
        .executableTarget(
            name: "Concerto",
            dependencies: ["LoopflowCore"],
            path: "Concerto",
            exclude: ["Info.plist", "Concerto.sdef"]
        ),
        .executableTarget(
            name: "Symphonia",
            dependencies: ["LoopflowCore"],
            path: "Symphonia"
        ),
        .testTarget(
            name: "ConcertoTests",
            dependencies: ["Concerto", "LoopflowCore", "ViewInspector"],
            path: "ConcertoTests"
        ),
        .testTarget(
            name: "SymphoniaTests",
            dependencies: ["Symphonia", "LoopflowCore"],
            path: "SymphoniaTests"
        ),
    ]
)
