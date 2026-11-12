#if os(macOS)
import Darwin
import Foundation

extension Pipe {
    /// A pipe that reaches only the child it is handed to. Terminal panes fork
    /// from Desktop at any moment; an inheritable end would reach every shell and
    /// owner started meanwhile, and a reader waiting for end of input would wait
    /// for as long as they run. The child's stdio copy is made by dup2, which
    /// clears close-on-exec.
    public static func uninherited() -> Pipe {
        let pipe = Pipe()
        for handle in [pipe.fileHandleForReading, pipe.fileHandleForWriting] {
            _ = fcntl(handle.fileDescriptor, F_SETFD, FD_CLOEXEC)
        }
        return pipe
    }
}
#endif
