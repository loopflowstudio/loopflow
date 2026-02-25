// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "LoopflowSwift",
    platforms: [
        .macOS(.v15),
        .iOS(.v18),
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
            path: "LoopflowCore",
            exclude: ["Info.plist"]
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
                .target(
                    name: "GhosttyKit",
                    condition: .when(platforms: [.macOS])
                ),
            ],
            path: "Concerto",
            exclude: ["Info.plist", "Concerto.sdef", "UX_DESIGN.md", "AppIcon.icns", "Services/Ghostty/README.md"],
            resources: [
                .copy("Fonts")
            ],
            swiftSettings: [
                .define("GHOSTTY_ENABLED", .when(platforms: [.macOS])),
            ],
            linkerSettings: [
                .linkedFramework("Carbon", .when(platforms: [.macOS])),
                .linkedFramework("QuartzCore", .when(platforms: [.macOS])),
                .linkedFramework("Metal", .when(platforms: [.macOS])),
                .linkedFramework("IOKit", .when(platforms: [.macOS])),
                .linkedLibrary("c++", .when(platforms: [.macOS])),
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
    ]
)
