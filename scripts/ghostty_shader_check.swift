// Headless checks of the production Metal color functions. No window or display link.
import Foundation
import Metal
import simd

enum CheckError: Error { case unavailable(String), failed(String) }
guard CommandLine.arguments.count == 2 else {
    throw CheckError.failed("Pass the patched Ghostty source directory")
}
guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue() else {
    throw CheckError.unavailable("No Metal device; run on a capable macOS runner")
}
let sourceURL = URL(fileURLWithPath: CommandLine.arguments[1])
    .appendingPathComponent("src/renderer/shaders/shaders.metal")
let source = try String(contentsOf: sourceURL, encoding: .utf8) + """

struct ContrastFixture { float4 fg; float4 bg; float4 options; };
kernel void terminal_contrast_check(
    constant ContrastFixture *fixtures [[buffer(0)]],
    texture2d<float, access::write> pixels [[texture(0)]],
    uint i [[thread_position_in_grid]]) {
    ContrastFixture f = fixtures[i];
    float4 fg = load_color(uchar4(f.fg), f.options.y > 0, true);
    float4 bg = load_color(uchar4(f.bg), f.options.y > 0, true);
    pixels.write(contrasted_color(f.options.x, fg, bg), uint2(i, 0));
    pixels.write(fg, uint2(i, 1));
}
"""
let library = try device.makeLibrary(source: source, options: nil)
let function = library.makeFunction(name: "terminal_contrast_check")!
let pipeline = try device.makeComputePipelineState(function: function)

struct Fixture {
    var fg: SIMD4<Float>
    var bg: SIMD4<Float>
    var options: SIMD4<Float>
}
func color(_ hex: UInt32, alpha: Float = 255) -> SIMD4<Float> {
    SIMD4(Float(hex >> 16 & 255), Float(hex >> 8 & 255), Float(hex & 255), alpha)
}
var fixtures: [Fixture] = []
// Input palette can be sRGB or P3; Metal always renders in linear P3.
for p3: Float in [0, 1] {
    for bg: UInt32 in [0x24211F, 0xFAF8F5, 0x888888] {
        for fg: UInt32 in [0x442222, 0x224422, 0x888888, 0xFAF8F5, 0x000000, bg] {
            for alpha: Float in [255, 128, 16] {
                for ratio: Float in [1, 3, 21] {
                    fixtures.append(Fixture(fg: color(fg, alpha: alpha), bg: color(bg),
                        options: SIMD4(ratio, p3, 0, 0)))
                }
            }
        }
    }
}
let buffer = fixtures.withUnsafeBytes { device.makeBuffer(bytes: $0.baseAddress!, length: $0.count) }!
let descriptor = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba32Float,
    width: fixtures.count, height: 2, mipmapped: false)
descriptor.storageMode = .shared
descriptor.usage = .shaderWrite
let texture = device.makeTexture(descriptor: descriptor)!
let command = queue.makeCommandBuffer()!
let encoder = command.makeComputeCommandEncoder()!
encoder.setComputePipelineState(pipeline)
encoder.setBuffer(buffer, offset: 0, index: 0)
encoder.setTexture(texture, index: 0)
encoder.dispatchThreads(MTLSize(width: fixtures.count, height: 1, depth: 1),
    threadsPerThreadgroup: MTLSize(width: 1, height: 1, depth: 1))
encoder.endEncoding()
command.commit()
command.waitUntilCompleted()
if let error = command.error { throw error }
// Each column holds one case: corrected color above its unchanged reference.
var pixels = [SIMD4<Float>](repeating: .zero, count: fixtures.count * 2)
pixels.withUnsafeMutableBytes {
    texture.getBytes($0.baseAddress!, bytesPerRow: $0.count / 2,
        from: MTLRegionMake2D(0, 0, fixtures.count, 2), mipmapLevel: 0)
}
let weights = SIMD3<Float>(0.22897456, 0.69173852, 0.07928691)
func rgb(_ value: SIMD4<Float>) -> SIMD3<Float> { SIMD3(value.x, value.y, value.z) }
func contrast(_ a: SIMD3<Float>, _ b: SIMD3<Float>) -> Float {
    let x = dot(a, weights) + 0.05
    let y = dot(b, weights) + 0.05
    return max(x, y) / min(x, y)
}
func linear(_ value: Float) -> Float {
    let v = value / 255
    return v <= 0.04045 ? v / 12.92 : pow((v + 0.055) / 1.055, 2.4)
}
// Independent conversion of the reference background to the shader's output space.
let srgbXYZ = simd_float3x3(rows: [SIMD3(0.4360747, 0.3850649, 0.1430804),
    SIMD3(0.2225045, 0.7168786, 0.0606169), SIMD3(0.0139322, 0.0971045, 0.7141733)])
let xyzP3 = simd_float3x3(rows: [SIMD3(2.40414768, -0.99010704, -0.39759019),
    SIMD3(-0.84239098, 1.79905954, 0.01597023), SIMD3(0.04838763, -0.09752546, 1.27393636)])
var limited = 0
for (i, f) in fixtures.enumerated() {
    let corrected = pixels[i]
    let original = pixels[fixtures.count + i]
    var bg = SIMD3(linear(f.bg.x), linear(f.bg.y), linear(f.bg.z))
    if f.options.y == 0 { bg = xyzP3 * srgbXYZ * bg }
    let base = bg * (1 - original.w)
    let before = contrast(rgb(original) + base, bg)
    let after = contrast(rgb(corrected) + base, bg)
    let attainable = max(contrast(base, bg), contrast(SIMD3(repeating: original.w) + base, bg))
    guard abs(corrected.w - original.w) < 0.00001 else { throw CheckError.failed("Changed faint alpha at \(i)") }
    if f.options.x == 1 || before >= f.options.x {
        guard corrected == original else { throw CheckError.failed("Changed passing/bypassed color at \(i)") }
    } else if attainable + 0.0001 < f.options.x {
        limited += 1
        guard abs(after - attainable) < 0.001 else { throw CheckError.failed("Did not reach best attainable contrast at \(i)") }
    } else {
        guard after + 0.001 >= f.options.x else { throw CheckError.failed("Contrast \(after) below \(f.options.x) at \(i)") }
    }
    for a in 0..<3 {
        for b in 0..<3 where original[a] > original[b] {
            guard corrected[a] + 0.00001 >= corrected[b] else { throw CheckError.failed("Reversed channel order at \(i)") }
        }
    }
}
func correctedColor(_ hex: UInt32) -> SIMD4<Float> {
    let index = fixtures.firstIndex {
        $0.fg == color(hex) && $0.bg == color(0x24211F) && $0.options == SIMD4(3, 0, 0, 0)
    }!
    return pixels[index]
}
guard correctedColor(0x442222) != correctedColor(0x224422) else {
    throw CheckError.failed("Red and green fixtures collapsed")
}

// Exercise the production background fragment into an offscreen render target.
// Its interior must remain unchanged while only the requested one-pixel edges light.
let render = MTLRenderPipelineDescriptor()
render.vertexFunction = library.makeFunction(name: "full_screen_vertex")
render.fragmentFunction = library.makeFunction(name: "cell_bg_fragment")
render.colorAttachments[0].pixelFormat = .rgba32Float
let renderPipeline = try device.makeRenderPipelineState(descriptor: render)
let targetDescriptor = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba32Float,
    width: 16, height: 16, mipmapped: false)
targetDescriptor.storageMode = .shared
targetDescriptor.usage = .renderTarget
let target = device.makeTexture(descriptor: targetDescriptor)!
// Uniform offsets match metal/shaders.zig's explicit C alignment.
let uniforms = device.makeBuffer(length: 144, options: .storageModeShared)!
memset(uniforms.contents(), 0, 144)
uniforms.contents().storeBytes(of: SIMD2<Float>(16, 16), toByteOffset: 64, as: SIMD2<Float>.self)
uniforms.contents().storeBytes(of: SIMD2<Float>(16, 16), toByteOffset: 72, as: SIMD2<Float>.self)
uniforms.contents().storeBytes(of: SIMD2<UInt16>(1, 1), toByteOffset: 80, as: SIMD2<UInt16>.self)
uniforms.contents().storeBytes(of: UInt32(0xFF1F2124), toByteOffset: 128, as: UInt32.self)
uniforms.contents().storeBytes(of: UInt8(1), toByteOffset: 134, as: UInt8.self)
var reference: SIMD4<Float>?
for alpha: UInt8 in [0, 180, 0] {
    var cell: [UInt8] = [48, 45, 43, 255, 15, alpha, 0, 0]
    let cells = device.makeBuffer(bytes: &cell, length: cell.count, options: .storageModeShared)!
    let pass = MTLRenderPassDescriptor()
    pass.colorAttachments[0].texture = target
    pass.colorAttachments[0].loadAction = .clear
    pass.colorAttachments[0].storeAction = .store
    let draw = queue.makeCommandBuffer()!
    let encoder = draw.makeRenderCommandEncoder(descriptor: pass)!
    encoder.setRenderPipelineState(renderPipeline)
    encoder.setFragmentBuffer(uniforms, offset: 0, index: 1)
    encoder.setFragmentBuffer(cells, offset: 0, index: 2)
    encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
    encoder.endEncoding()
    draw.commit()
    draw.waitUntilCompleted()
    if let error = draw.error { throw error }
    var shown = [SIMD4<Float>](repeating: .zero, count: 256)
    shown.withUnsafeMutableBytes {
        target.getBytes($0.baseAddress!, bytesPerRow: 16 * MemoryLayout<SIMD4<Float>>.stride,
                        from: MTLRegionMake2D(0, 0, 16, 16), mipmapLevel: 0)
    }
    let center = shown[8 * 16 + 8]
    if let reference {
        guard center == reference else { throw CheckError.failed("Feedback changed cell interiors") }
    } else { reference = center }
    guard alpha == 0 ? shown[0] == center : shown[0] != center else {
        throw CheckError.failed("Feedback edge did not appear/expire")
    }
}
print("Terminal contrast: \(fixtures.count) Metal fixtures passed; \(limited) unattainable ratios explicitly verified")
