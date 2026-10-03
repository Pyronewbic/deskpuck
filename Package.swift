// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "Deskpuck",
    platforms: [.macOS(.v13)],
    products: [
        .executable(name: "Deskpuck", targets: ["Deskpuck"]),
        .executable(name: "deskpuck-cli", targets: ["deskpuck-cli"]),
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
        .executableTarget(name: "deskpuck-cli", dependencies: ["DeskpuckCore"]),
        .executableTarget(name: "Deskpuck", dependencies: ["DeskpuckCore"]),
    ],
    cxxLanguageStandard: .cxx17
)
