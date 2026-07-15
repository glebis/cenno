// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "CennoVoice",
    platforms: [.macOS(.v13)],
    products: [
        .library(name: "CennoVoice",  type: .static, targets: ["CennoVoice"]),
        .library(name: "CennoRelay",  type: .static, targets: ["CennoRelay"]),
        .library(name: "CennoScreenContext", type: .static, targets: ["CennoScreenContext"]),
    ],
    dependencies: [
        .package(url: "https://github.com/Brendonovich/swift-rs", from: "1.0.6")
    ],
    targets: [
        .target(
            name: "CennoVoice",
            dependencies: [
                .product(name: "SwiftRs", package: "swift-rs")
            ],
            linkerSettings: [
                .linkedFramework("Speech"),
                .linkedFramework("AVFoundation"),
                .linkedFramework("Foundation"),
            ]
        ),
        .target(
            name: "CennoRelay",
            dependencies: [
                .product(name: "SwiftRs", package: "swift-rs")
            ],
            linkerSettings: [
                .linkedFramework("CloudKit"),
                .linkedFramework("Foundation"),
            ]
        ),
        .target(
            name: "CennoScreenContext",
            dependencies: [
                .product(name: "SwiftRs", package: "swift-rs")
            ],
            linkerSettings: [
                .linkedFramework("ApplicationServices"),
                .linkedFramework("AppKit"),
                .linkedFramework("Foundation"),
            ]
        ),
        .testTarget(
            name: "CennoScreenContextTests",
            dependencies: ["CennoScreenContext"]
        ),
    ]
)
