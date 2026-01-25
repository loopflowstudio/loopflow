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
        .binaryTarget(
            name: "GhosttyKit",
            url: "https://bin.loopflow.studio/GhosttyKit-061a0ae.xcframework.zip",
            checksum: "1bbc50b79356ccf2a1e22b96d34a1fcd1c4b4a31c6e88756f9b3683c1a5532ca"
        ),
        .executableTarget(
            name: "Concerto",
            dependencies: [
                "LoopflowCore",
                "GhosttyKit"
            ],
            path: "Concerto",
            exclude: ["Info.plist", "Concerto.sdef"],
            swiftSettings: [
                .define("GHOSTTY_ENABLED")
            ],
            linkerSettings: [
                .linkedFramework("Carbon"),
                .linkedFramework("QuartzCore"),
                .linkedFramework("Metal"),
                .linkedFramework("IOKit"),
                .linkedLibrary("c++")
            ]
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
