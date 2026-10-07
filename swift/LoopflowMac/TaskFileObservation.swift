#if os(macOS)
import CoreServices
import Darwin
import Foundation

/// Observe names recursively, so atomic replacement and recreated directories
/// keep working. Events return relative paths (empty for a full rescan), never contents.
@MainActor
final class TaskFileObservation {
    private let root: String
    /// A linked worktree keeps its index and HEAD outside the checkout.
    private let metadata: String?
    private var watched: Set<String> = []
    private let resources = Resources()
    private let changed: @MainActor ([String]) -> Void

    init(path: String, changed: @escaping @MainActor ([String]) -> Void) throws {
        self.changed = changed
        let root = URL(fileURLWithPath: path).resolvingSymlinksInPath().path
        self.root = root
        metadata = (try? String(contentsOfFile: root + "/.git", encoding: .utf8)).flatMap { pointer in
            guard pointer.hasPrefix("gitdir:") else { return nil }
            let target = pointer.dropFirst("gitdir:".count).trimmingCharacters(in: .whitespacesAndNewlines)
            // Events name the real path; `resolvingSymlinksInPath` drops `/private`.
            guard let real = realpath(URL(fileURLWithPath: target, relativeTo: URL(fileURLWithPath: root)).path, nil)
            else { return nil }
            defer { free(real) }
            return String(cString: real)
        }
        let callback = Callback(observation: self)
        defer { withExtendedLifetime(callback) {} }
        var context = FSEventStreamContext(
            version: 0, info: Unmanaged.passUnretained(callback).toOpaque(),
            retain: { info in
                guard let info else { return nil }
                _ = Unmanaged<Callback>.fromOpaque(info).retain()
                return info
            },
            release: { info in
                guard let info else { return }
                Unmanaged<Callback>.fromOpaque(info).release()
            }, copyDescription: nil)
        let flags = UInt32(kFSEventStreamCreateFlagFileEvents | kFSEventStreamCreateFlagUseCFTypes
                           | kFSEventStreamCreateFlagNoDefer | kFSEventStreamCreateFlagWatchRoot)
        guard let stream = FSEventStreamCreate(nil, { _, context, count, paths, eventFlags, _ in
            guard let context else { return }
            let callback = Unmanaged<Callback>.fromOpaque(context).takeUnretainedValue()
            let names = unsafeBitCast(paths, to: NSArray.self) as! [String]
            let dropped = UInt32(kFSEventStreamEventFlagMustScanSubDirs | kFSEventStreamEventFlagUserDropped
                                 | kFSEventStreamEventFlagKernelDropped | kFSEventStreamEventFlagRootChanged)
            let rescan = (0..<count).contains { eventFlags[$0] & dropped != 0 }
            MainActor.assumeIsolated {
                guard let observation = callback.observation else { return }
                observation.notify(rescan ? [observation.root] : names)
            }
        }, &context, ([root] + (metadata.map { [$0] } ?? [])) as CFArray, FSEventStreamEventId(kFSEventStreamEventIdSinceNow), 0.1, flags) else {
            throw CocoaError(.fileReadUnknown)
        }
        resources.stream = stream
        FSEventStreamSetDispatchQueue(stream, .main)
        guard FSEventStreamStart(stream) else {
            FSEventStreamInvalidate(stream)
            FSEventStreamRelease(stream)
            resources.stream = nil
            throw CocoaError(.fileReadUnknown)
        }
    }

    private func notify(_ paths: [String]) {
        let prefix = root + "/"
        changed(paths.compactMap { path in
            if path == root { return "" }
            // Reported as if the metadata sat inside the checkout, as a main checkout's does.
            if let metadata, path == metadata || path.hasPrefix(metadata + "/") {
                return ".git" + path.dropFirst(metadata.count)
            }
            return path.hasPrefix(prefix) ? String(path.dropFirst(prefix.count)) : nil
        })
    }

    /// Direct vnode events cover open documents promptly, including temporary
    /// volumes excluded by FSEvents. Parent observation reconnects replaced names.
    func watch(_ relative: String) {
        watched.insert(relative)
        attachAncestors(relative)
    }

    private func attachAncestors(_ relative: String) {
        var path = URL(fileURLWithPath: root).appendingPathComponent(relative)
        while path.path == root || path.path.hasPrefix(root + "/") {
            attach(path.path)
            if path.path == root { break }
            path.deleteLastPathComponent()
        }
    }

    private func attach(_ path: String) {
        guard resources.sources[path] == nil else { return }
        let fd = open(path, O_EVTONLY | O_CLOEXEC)
        guard fd >= 0 else { return } // A retained parent observes later recreation.
        let source = DispatchSource.makeFileSystemObjectSource(fileDescriptor: fd,
            eventMask: [.write, .extend, .attrib, .rename, .delete, .revoke], queue: .main)
        resources.sources[path] = source
        source.setEventHandler { [weak self] in
            MainActor.assumeIsolated {
                guard let self else { return }
                if let source = self.resources.sources[path], !source.data.intersection([.rename, .delete, .revoke]).isEmpty {
                    source.cancel()
                    self.resources.sources[path] = nil
                }
                for relative in self.watched { self.attachAncestors(relative) }
                self.notify([path])
            }
        }
        source.setCancelHandler { close(fd) }
        source.resume()
    }

    private final class Callback {
        weak var observation: TaskFileObservation?

        init(observation: TaskFileObservation) {
            self.observation = observation
        }
    }

    // Only the observer mutates these resources, on the main actor. Their
    // cancellation APIs can run on whichever thread releases the last owner.
    private final class Resources {
        var sources: [String: DispatchSourceFileSystemObject] = [:]
        var stream: FSEventStreamRef?

        deinit {
            for source in sources.values { source.cancel() }
            if let stream {
                FSEventStreamStop(stream)
                FSEventStreamInvalidate(stream)
                FSEventStreamRelease(stream)
            }
        }
    }
}
#endif
