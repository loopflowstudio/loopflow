// swift-tools-version: 6.2

import PackageDescription

// SwiftPM builds the cross-platform `Loopflow` library and the macOS app. The
// app links Loopflow's Rust library, which `scripts/loopflow-dev.py ffi` builds
// into the Cargo target directory first.
let cargoTarget = Context.packageDirectory + "/../target"

let package = Package(
    name: "LoopflowSwift",
    platforms: [
        .macOS(.v26),
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
                .copy("Fonts"),
                .copy("Themes")
            ]
        ),
        .systemLibrary(name: "LoopflowFFI", path: "LoopflowFFI"),
        .binaryTarget(
            name: "GhosttyKit",
            url: "https://bin.loopflow.studio/GhosttyKit-a60e9e2-lf7-theme-readback1.xcframework.zip",
            checksum: "19d6f6e2f93cf3d967aaaa9761c6ee1995683381ac432598d7b0496610934ce9"
        ),
        .executableTarget(
            name: "LoopflowMac",
            dependencies: [
                "Loopflow",
                .target(name: "LoopflowFFI", condition: .when(platforms: [.macOS])),
                .target(name: "GhosttyKit", condition: .when(platforms: [.macOS])),
            ],
            path: "LoopflowMac",
            exclude: [
                "Info.plist",
                "Loopflow.entitlements",
                "AppIcon.icns",
                "logo.svg",
                "dmg-background.png",
            ],
            resources: [
                .copy("GhosttyResources"),
                .copy("ThirdPartyNotices")
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
                // What `cargo rustc -p loopflow-ffi -- --print native-static-libs` names.
                .linkedFramework("CoreServices", .when(platforms: [.macOS])),
                .linkedFramework("Security", .when(platforms: [.macOS])),
                .linkedFramework("SystemConfiguration", .when(platforms: [.macOS])),
                .linkedLibrary("iconv", .when(platforms: [.macOS])),
                .linkedLibrary("proc", .when(platforms: [.macOS])),
                .unsafeFlags(["-L", cargoTarget + "/debug"], .when(platforms: [.macOS], configuration: .debug)),
                .unsafeFlags(["-L", cargoTarget + "/release"], .when(platforms: [.macOS], configuration: .release)),
            ]
        ),
        .testTarget(
            name: "LoopflowTests",
            dependencies: ["LoopflowMac", "Loopflow", "ViewInspector"],
            path: "LoopflowTests"
        ),
    ]
)
