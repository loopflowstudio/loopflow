// Fixed-image observer diagnosis. No workspace, terminal, provider or window is opened.
// Compile with xcrun swiftc; run under the containment policy in the benchmark guide.
import AppKit
import CryptoKit
import Darwin
import Vision

func residentBytes() -> UInt64 {
    var info = mach_task_basic_info()
    var count = mach_msg_type_number_t(MemoryLayout<mach_task_basic_info>.size / MemoryLayout<natural_t>.size)
    let result = withUnsafeMutablePointer(to: &info) {
        $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
            task_info(mach_task_self_, task_flavor_t(MACH_TASK_BASIC_INFO), $0, &count)
        }
    }
    precondition(result == KERN_SUCCESS, "Cannot measure this probe's resident memory")
    return info.resident_size
}

func emit(_ value: [String: Any]) throws {
    var data = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
    data.append(10)
    try FileHandle.standardOutput.write(contentsOf: data)
}

func recognize(_ image: CGImage) throws -> [String: Any] {
    var labels: [String] = []
    var result: [String: Any] = [:]
    for (name, level): (String, VNRequestTextRecognitionLevel) in [("fast", .fast), ("accurate", .accurate)] {
        let start = DispatchTime.now().uptimeNanoseconds
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = level
        request.recognitionLanguages = ["en-US"]
        request.usesLanguageCorrection = false
        try VNImageRequestHandler(cgImage: image).perform([request])
        labels += (request.results ?? []).compactMap { $0.topCandidates(1).first?.string }
        result[name + "_ms"] = Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000
        result[name + "_rss_bytes"] = residentBytes()
    }
    // Retain equality evidence without copying private rendered text into receipts.
    result["labels_sha256"] = SHA256.hash(data: try JSONEncoder().encode(labels)).description
    result["labels"] = labels.count
    return result
}

precondition(CommandLine.arguments.count == 3, "Usage: observe-pixels IMAGE pooled|unpooled")
let mode = CommandLine.arguments[2]
precondition(["pooled", "unpooled"].contains(mode), "Unknown mode")
let bytes = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
guard let image = NSBitmapImageRep(data: bytes)?.cgImage else { fatalError("Cannot decode capture") }
try emit(["event": "start", "pid": getpid(), "mode": mode,
          "image_sha256": SHA256.hash(data: bytes).description, "rss_bytes": residentBytes(),
          "width": image.width, "height": image.height, "iterations": 40,
          "rss_limit_bytes": 512 * 1024 * 1024])
var completed = 0
try autoreleasepool {
    for index in 1...40 {
        let start = DispatchTime.now().uptimeNanoseconds
        var result = try mode == "pooled" ? autoreleasepool { try recognize(image) } : recognize(image)
        result["event"] = "observation"
        result["iteration"] = index
        result["duration_ms"] = Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000
        result["rss_bytes"] = residentBytes()
        try emit(result)
        completed = index
        if residentBytes() > 512 * 1024 * 1024 { break }
    }
}
try emit(["event": "drained", "rss_bytes": residentBytes(), "completed": completed])
if completed != 40 { exit(1) }
