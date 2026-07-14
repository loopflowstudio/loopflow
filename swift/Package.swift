// swift-tools-version: 6.0

import PackageDescription

// SwiftPM builds the cross-platform `Loopflow` library and the macOS app.
let package = Package(
    name: "LoopflowSwift",
    platforms: [
        .macOS(.v15),
        .iOS(.v18),
    ],
    products: [
        .library(name: "Loopflow", targets: ["Loopflow"]),
        .executable(name: "LoopflowMac", targets: ["LoopflowMac"]),
    ],
    dependencies: [
        .package(url: "https://github.com/nalexn/ViewInspector.git", from: "0.10.0"),
    ],
    targets: [
        .target(
            name: "Loopflow",
            dependencies: [],
            path: "Loopflow",
            exclude: ["Info.plist"],
            resources: [
                .copy("Fonts")
            ]
        ),
        .binaryTarget(
            name: "GhosttyKit",
            url: "https://bin.loopflow.studio/GhosttyKit-4c83872.xcframework.zip",
            checksum: "b0e75385d69477d92f673962f2361642b1a22b228ad249036cbef53c0788a74d"
        ),
        .executableTarget(
            name: "LoopflowMac",
            dependencies: [
                "Loopflow",
                .target(
                    name: "GhosttyKit",
                    condition: .when(platforms: [.macOS])
                ),
            ],
            path: "LoopflowMac",
            exclude: [
                "Info.plist",
                "Loopflow.sdef",
                "Loopflow.entitlements",
                "UX_DESIGN.md",
                "AppIcon.icns",
                "logo.svg",
                "dmg-background.png",
                "Services/Ghostty/README.md",
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
            name: "LoopflowTests",
            dependencies: ["LoopflowMac", "Loopflow", "ViewInspector"],
            path: "LoopflowTests"
        ),
    ]
)
