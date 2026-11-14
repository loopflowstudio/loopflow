// Export the Desktop artwork as an opaque iOS app icon.
// Run from the repository root: xcrun swift scripts/ios-icon.swift
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
let sourceURL = root.appendingPathComponent("swift/LoopflowMac/AppIcon.icns")
let outputURL = root.appendingPathComponent("swift/LoopflowiOS/Assets.xcassets/AppIcon.appiconset/AppIcon.png")
guard let source = CGImageSourceCreateWithURL(sourceURL as CFURL, nil) else {
    fatalError("Cannot read Desktop's app icon")
}
let images = (0..<CGImageSourceGetCount(source)).compactMap {
    CGImageSourceCreateImageAtIndex(source, $0, nil)
}
guard let image = images.max(by: { $0.width < $1.width }), image.width == 1024, image.height == 1024,
      let colorSpace = CGColorSpace(name: CGColorSpace.sRGB),
      let context = CGContext(data: nil, width: image.width, height: image.height,
          bitsPerComponent: 8, bytesPerRow: image.width * 4, space: colorSpace,
          bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue) else {
    fatalError("Desktop's icon must include 1024×1024 artwork")
}
context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
guard let opaque = context.makeImage(),
      let destination = CGImageDestinationCreateWithURL(outputURL as CFURL, UTType.png.identifier as CFString, 1, nil) else {
    fatalError("Cannot create the iOS icon")
}
CGImageDestinationAddImage(destination, opaque, nil)
guard CGImageDestinationFinalize(destination) else { fatalError("Cannot write the iOS icon") }
