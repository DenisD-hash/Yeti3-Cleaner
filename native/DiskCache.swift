import Foundation
import SQLite3

struct DiskSnapshot {
    let entries: [Entry]
    let savedAt: Date
    let complete: Bool
}
// One connection per scan, used only on that scan's background queue.
final class DiskCache {
    private var db: OpaquePointer?
    private let transient = unsafeBitCast(-1, to: sqlite3_destructor_type.self)
    init(path: String) throws {
        guard sqlite3_open_v2(path, &db, SQLITE_OPEN_READWRITE | SQLITE_OPEN_FULLMUTEX, nil) == SQLITE_OK else {
            let error = failure(); sqlite3_close(db); db = nil; throw error
        }
        sqlite3_busy_timeout(db, 1000)
        try execute("CREATE TABLE IF NOT EXISTS disk_map_cache(root TEXT PRIMARY KEY, scan_started REAL NOT NULL, saved_at REAL NOT NULL, complete INTEGER NOT NULL, entries BLOB NOT NULL)")
    }
    deinit { sqlite3_close(db) }
    private func failure() -> NSError {
        NSError(domain: "YetiDiskCache", code: 1, userInfo: [NSLocalizedDescriptionKey: db.map { String(cString: sqlite3_errmsg($0)) } ?? "SQLite недоступен"])
    }
    private func execute(_ sql: String) throws {
        guard sqlite3_exec(db, sql, nil, nil, nil) == SQLITE_OK else { throw failure() }
    }
    private func prepare(_ sql: String) throws -> OpaquePointer {
        var statement: OpaquePointer?
        guard sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK, let statement else { throw failure() }
        return statement
    }
    func load(_ root: URL) throws -> DiskSnapshot? {
        let statement = try prepare("SELECT saved_at, complete, entries FROM disk_map_cache WHERE root=?1")
        defer { sqlite3_finalize(statement) }
        sqlite3_bind_text(statement, 1, root.standardizedFileURL.path, -1, transient)
        let result = sqlite3_step(statement)
        if result == SQLITE_DONE { return nil }
        guard result == SQLITE_ROW, let bytes = sqlite3_column_blob(statement, 2) else { throw failure() }
        let data = Data(bytes: bytes, count: Int(sqlite3_column_bytes(statement, 2)))
        return DiskSnapshot(entries: try JSONDecoder().decode([Entry].self, from: data), savedAt: Date(timeIntervalSince1970: sqlite3_column_double(statement, 0)), complete: sqlite3_column_int(statement, 1) != 0)
    }
    func save(_ root: URL, entries: [Entry], started: Date, complete: Bool) throws {
        let data = try JSONEncoder().encode(entries)
        let statement = try prepare("INSERT INTO disk_map_cache VALUES(?1,?2,?3,?4,?5) ON CONFLICT(root) DO UPDATE SET scan_started=excluded.scan_started,saved_at=excluded.saved_at,complete=excluded.complete,entries=excluded.entries WHERE excluded.scan_started>=disk_map_cache.scan_started")
        defer { sqlite3_finalize(statement) }
        sqlite3_bind_text(statement, 1, root.standardizedFileURL.path, -1, transient)
        sqlite3_bind_double(statement, 2, started.timeIntervalSince1970)
        sqlite3_bind_double(statement, 3, Date().timeIntervalSince1970)
        sqlite3_bind_int(statement, 4, complete ? 1 : 0)
        _ = data.withUnsafeBytes { sqlite3_bind_blob(statement, 5, $0.baseAddress, Int32($0.count), transient) }
        guard sqlite3_step(statement) == SQLITE_DONE else { throw failure() }
        // Keep navigation history bounded instead of growing the database forever.
        try execute("DELETE FROM disk_map_cache WHERE root NOT IN (SELECT root FROM disk_map_cache ORDER BY saved_at DESC LIMIT 128)")
    }
}
