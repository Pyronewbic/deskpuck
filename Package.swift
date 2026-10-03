// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "JoyMouse",
    platforms: [.macOS(.v13)],
    products: [
        .executable(name: "JoyMouse", targets: ["JoyMouse"]),
        .executable(name: "joymouse-cli", targets: ["joymouse-cli"]),
    ],
    targets: [
        .target(
            name: "JoyMouseCore",
            linkerSettings: [
                .linkedFramework("ApplicationServices"),
                .linkedFramework("CoreBluetooth"),
                .linkedFramework("Foundation"),
            ]
        ),
        .executableTarget(name: "joymouse-cli", dependencies: ["JoyMouseCore"]),
        .executableTarget(name: "JoyMouse", dependencies: ["JoyMouseCore"]),
    ],
    cxxLanguageStandard: .cxx17
)
