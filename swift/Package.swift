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
    ],
    dependencies: [
        .package(url: "https://github.com/nalexn/ViewInspector.git", from: "0.10.0"),
        .package(url: "https://github.com/argmaxinc/WhisperKit.git", .upToNextMinor(from: "0.9.0")),
    ],
    targets: [
        .target(
            name: "LoopflowCore",
            dependencies: [
                .product(name: "WhisperKit", package: "WhisperKit"),
            ],
            path: "LoopflowCore",
            exclude: ["Info.plist"]
        ),
        .binaryTarget(
            name: "GhosttyKit",
            url: "https://bin.loopflow.studio/GhosttyKit-4c83872.xcframework.zip",
            checksum: "b0e75385d69477d92f673962f2361642b1a22b228ad249036cbef53c0788a74d"
        ),
        .executableTarget(
            name: "Concerto",
            dependencies: [
                "LoopflowCore",
                .product(name: "WhisperKit", package: "WhisperKit"),
                .target(
                    name: "GhosttyKit",
                    condition: .when(platforms: [.macOS])
                ),
            ],
            path: "Concerto",
            exclude: ["Info.plist", "Concerto.sdef", "Concerto.entitlements", "UX_DESIGN.md", "AppIcon.icns", "logo.svg", "dmg-background.png", "Services/Ghostty/README.md"],
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
        .testTarget(
            name: "ConcertoTests",
            dependencies: ["Concerto", "LoopflowCore", "ViewInspector"],
            path: "ConcertoTests"
        ),
    ]
)
