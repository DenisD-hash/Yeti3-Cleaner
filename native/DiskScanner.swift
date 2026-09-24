import Foundation
import Darwin

struct Entry: Identifiable, Codable {
    var id: String { url.path }
    let url: URL
    let bytes: Int64
    let directory: Bool
    let errors: Int
}
final class Cancellation: @unchecked Sendable {
    private let lock = NSLock()
    private var value = false
    var cancelled: Bool { lock.lock(); defer { lock.unlock() }; return value }
    func cancel() { lock.lock(); value = true; lock.unlock() }
}
// Enumerate metadata only. Symlinks and other mounted volumes are never followed.
// Each navigation scans one subtree, keeping only its immediate children in memory.
func scan(_ root: URL, token: Cancellation, progress: @escaping (Int) -> Void, snapshot: ([Entry]) -> Void = { _ in }, publishInterval: TimeInterval = 0.15, entryProgress: (Entry, Int) -> Void = { _, _ in }) throws -> [Entry] {
    let fm = FileManager.default
    let keys: Set<URLResourceKey> = [.isDirectoryKey, .isSymbolicLinkKey, .fileSizeKey]
    let children = try fm.contentsOfDirectory(at: root, includingPropertiesForKeys: Array(keys))
    var result: [Entry] = []; var count = 0
    var lastEmission = ProcessInfo.processInfo.systemUptime
    func publish(_ current: Entry? = nil, force: Bool = false) {
        let now = ProcessInfo.processInfo.systemUptime
        guard force || now - lastEmission >= publishInterval else { return }
        lastEmission = now
        var partial = result
        if let current { partial.append(current) }
        snapshot(partial.sorted { $0.bytes > $1.bytes })
        progress(count)
    }
    publish(force: true)
    for child in children {
        if token.cancelled { break }
        var errors = 0; var bytes: Int64 = 0; var directory = false
        do {
            let v = try child.resourceValues(forKeys: keys)
            directory = v.isDirectory == true
            if v.isSymbolicLink == true { continue }
            if directory {
                if root.path == "/" && child.lastPathComponent == "Volumes" { continue }
                entryProgress(Entry(url: child, bytes: 0, directory: true, errors: 0), count)
                let rootVolume = try device(child)
                if let walk = fm.enumerator(at: child, includingPropertiesForKeys: Array(keys), options: [], errorHandler: { _, _ in errors += 1; return true }) {
                    for case let item as URL in walk {
                        if token.cancelled { break }
                        do {
                            let a = try item.resourceValues(forKeys: keys)
                            if a.isSymbolicLink == true { continue }
                            if a.isDirectory == true, try device(item) != rootVolume { walk.skipDescendants(); continue }
                            if a.isDirectory != true { bytes += Int64(a.fileSize ?? 0) }
                        } catch { errors += 1 }
                        count += 1
                        entryProgress(Entry(url: child, bytes: bytes, directory: directory, errors: errors), count)
                        if count % 64 == 0 {
                            publish(Entry(url: child, bytes: bytes, directory: directory, errors: errors))
                        }
                    }
                } else { errors += 1 }
            } else { bytes = Int64(v.fileSize ?? 0) }
        } catch { errors += 1 }
        result.append(Entry(url: child, bytes: bytes, directory: directory, errors: errors))
        count += 1
        entryProgress(Entry(url: child, bytes: bytes, directory: directory, errors: errors), count)
        publish()
    }
    publish(force: true)
    return result.sorted { $0.bytes > $1.bytes }
}


private func device(_ url: URL) throws -> dev_t {
    var info = stat()
    guard url.path.withCString({ lstat($0, &info) }) == 0 else { throw POSIXError(.EACCES) }
    return info.st_dev
}
