// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "Deskpuck",
    platforms: [.macOS(.v13)],
    products: [
        .executable(name: "Deskpuck", targets: ["Deskpuck"]),
    ],
    targets: [
        .target(
            name: "DeskpuckCore",
            linkerSettings: [
                .linkedFramework("ApplicationServices"),
                .linkedFramework("CoreBluetooth"),
                .linkedFramework("Foundation"),
            ]
        ),
        // The Rust core's static library, built by scripts/build-rust.sh.
        .systemLibrary(name: "DeskpuckFFI", path: "Sources/DeskpuckFFI"),
        .executableTarget(
            name: "Deskpuck",
            dependencies: ["DeskpuckFFI"],
            linkerSettings: [
                .unsafeFlags(["-L", Context.packageDirectory + "/rust/target/release"]),
                // What the Rust static library needs (cargo --print native-static-libs).
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
    ],
    cxxLanguageStandard: .cxx17
)
