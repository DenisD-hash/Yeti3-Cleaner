import Foundation
import SQLite3
@main struct CacheTests {
    static func main() throws {
        let path = FileManager.default.temporaryDirectory.appendingPathComponent("yeti-cache-" + UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: path) }
        var connection: OpaquePointer?
        precondition(sqlite3_open(path.path, &connection) == SQLITE_OK)
        sqlite3_close(connection)
        let root = URL(fileURLWithPath: "/test")
        let entries = [Entry(url: root.appendingPathComponent("a"), bytes: 123, directory: true, errors: 2)]
        let start = Date()
        do {
            let cache = try DiskCache(path: path.path)
            let missing = try cache.load(root); precondition(missing == nil)
            try cache.save(root, entries: entries, started: start, complete: false)
        }
        let reopened = try DiskCache(path: path.path)
        let partial = try reopened.load(root)!
        precondition(!partial.complete && partial.entries[0].bytes == 123 && partial.entries[0].errors == 2)
        try reopened.save(root, entries: [], started: start.addingTimeInterval(-1), complete: true)
        let retained = try reopened.load(root)!; precondition(retained.entries.count == 1)
        try reopened.save(root, entries: [], started: start.addingTimeInterval(1), complete: true)
        let empty = try reopened.load(root)!; precondition(empty.complete && empty.entries.isEmpty)
        for i in 0..<129 { try reopened.save(URL(fileURLWithPath: "/folder/\(i)"), entries: entries, started: start, complete: true) }
        let pruned = try reopened.load(root); precondition(pruned == nil)
        print("Cache: persistence across reopen, partial snapshots, stale-writer rejection, empty completed folders and bounded history passed")
    }
}
