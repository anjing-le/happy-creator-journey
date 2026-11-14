// Export the original opaque artwork pixels through the imagegen alpha matte.
// Usage: swift export-icon.swift original.png imagegen-matte.png output.png
import Foundation
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers

func load(_ path: String) throws -> (Int, Int, [UInt8]) {
    guard let source = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, nil),
          let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
        throw NSError(domain: "IconExport", code: 1)
    }
    let w = image.width, h = image.height
    var pixels = [UInt8](repeating: 0, count: w * h * 4)
    let valid = pixels.withUnsafeMutableBytes { buffer -> Bool in
        guard let context = CGContext(data: buffer.baseAddress, width: w, height: h, bitsPerComponent: 8,
            bytesPerRow: w * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
        context.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
        return true
    }
    guard valid else { throw NSError(domain: "IconExport", code: 2) }
    return (w, h, pixels)
}
let args = CommandLine.arguments
precondition(args.count == 4)
let (w, h, original) = try load(args[1])
let (mw, mh, matte) = try load(args[2])
precondition(w == mw && h == mh, "Matte must align with original pixels")
// Protect the antialiased boundary: generated matte edges must not acquire
// cream pixels from the original paper background when the matte shifts slightly.
let stride = w + 1
var outside = [Int](repeating: 0, count: (w + 1) * (h + 1))
for y in 0..<h {
    var row = 0
    for x in 0..<w {
        row += matte[(y * w + x) * 4 + 3] < 245 ? 1 : 0
        outside[(y + 1) * stride + x + 1] = outside[y * stride + x + 1] + row
    }
}
var output = matte
var preserved = 0
for i in Swift.stride(from: 0, to: output.count, by: 4) {
    let alpha = matte[i+3]
    let x = (i / 4) % w, y = (i / 4) / w
    let x0 = max(0, x - 8), y0 = max(0, y - 8)
    let x1 = min(w, x + 9), y1 = min(h, y + 9)
    let outsideCount = outside[y1 * stride + x1] - outside[y0 * stride + x1]
        - outside[y1 * stride + x0] + outside[y0 * stride + x0]
    if alpha >= 245 && outsideCount == 0 && original[i+3] == 255 {
        output[i] = original[i]; output[i+1] = original[i+1]; output[i+2] = original[i+2]; output[i+3] = 255
        preserved += 1
    } else if alpha == 0 {
        output[i] = 0; output[i+1] = 0; output[i+2] = 0
    }
}
let data = Data(output)
let provider = CGDataProvider(data: data as CFData)!
let image = CGImage(width: w, height: h, bitsPerComponent: 8, bitsPerPixel: 32,
    bytesPerRow: w * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
    bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
    provider: provider, decode: nil, shouldInterpolate: true, intent: .defaultIntent)!
let destination = CGImageDestinationCreateWithURL(URL(fileURLWithPath: args[3]) as CFURL, UTType.png.identifier as CFString, 1, nil)!
CGImageDestinationAddImage(destination, image, nil)
precondition(CGImageDestinationFinalize(destination), "PNG export failed")
print("Exported RGBA PNG: \(w)x\(h); restored \(preserved) opaque original pixels")
