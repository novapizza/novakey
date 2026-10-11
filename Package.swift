// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "NovaKey",
    platforms: [
        .macOS(.v14)
    ],
    dependencies: [
        .package(url: "https://github.com/sparkle-project/Sparkle", from: "2.6.0"),
    ],
    targets: [
        .executableTarget(
            name: "NovaKey",
            dependencies: [
                .product(name: "Sparkle", package: "Sparkle"),
            ],
            path: "Sources/NovaKey",
            linkerSettings: [
                .linkedFramework("Cocoa"),
                .linkedFramework("Carbon"),
                // Sparkle is embedded at NovaKey.app/Contents/Frameworks by build.sh.
                // SPM links it as @rpath/Sparkle.framework but adds no rpath for the
                // app-bundle Frameworks dir, so dyld can't find it at runtime.
                .unsafeFlags(["-Xlinker", "-rpath", "-Xlinker", "@executable_path/../Frameworks"]),
            ]
        ),
    ]
)
