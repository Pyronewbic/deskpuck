// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "Deskpuck",
    platforms: [.macOS(.v13)],
    products: [
        .executable(name: "Deskpuck", targets: ["Deskpuck"]),
    ],
    targets: [
        .systemLibrary(name: "DeskpuckFFI", path: "Sources/DeskpuckFFI"),
        .executableTarget(
            name: "Deskpuck",
            dependencies: ["DeskpuckFFI"],
            linkerSettings: [
                .unsafeFlags(["-L", Context.packageDirectory + "/rust/target/release"]),
                .linkedFramework("AppKit"),
                .linkedFramework("ApplicationServices"),
                .linkedFramework("CoreBluetooth"),
                .linkedFramework("CoreFoundation"),
                .linkedFramework("CoreGraphics"),
                .linkedFramework("Foundation"),
                .linkedLibrary("iconv"),
                .linkedLibrary("objc"),
            ]
        ),
    ]
)
