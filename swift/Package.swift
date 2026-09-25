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
            url: "https://bin.loopflow.studio/GhosttyKit-4c83872-lf1.xcframework.zip",
            checksum: "8d65702d5ceb7e751a794620f7c15dd8a6a6f90d367e301a43d10fb839ade9a5"
        ),
        .executableTarget(
            name: "LoopflowMac",
            dependencies: [
                "Loopflow",
                .target(name: "GhosttyKit", condition: .when(platforms: [.macOS])),
            ],
            path: "LoopflowMac",
            exclude: [
                "Info.plist",
                "Loopflow.sdef",
                "Loopflow.entitlements",
                "AppIcon.icns",
                "logo.svg",
                "dmg-background.png",
            ],
            resources: [
                .copy("GhosttyResources")
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
