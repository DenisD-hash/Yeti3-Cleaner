import Foundation
@main struct ScannerTests {
    static func main() throws {
        let fm = FileManager.default
        let root = fm.temporaryDirectory.appendingPathComponent("yeti3-scanner-test-" + UUID().uuidString)
        try fm.createDirectory(at: root.appendingPathComponent("folder/nested"), withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: root) }
        try Data(repeating: 1, count: 123).write(to: root.appendingPathComponent("folder/nested/a"))
        try Data(repeating: 2, count: 456).write(to: root.appendingPathComponent("b"))
        try fm.createSymbolicLink(at: root.appendingPathComponent("folder/cycle"), withDestinationURL: root)
        try fm.createSymbolicLink(at: root.appendingPathComponent("link"), withDestinationURL: root.appendingPathComponent("b"))
        var snapshots: [[Entry]] = []
        let entries = try scan(root, token: Cancellation(), progress: { _ in }, snapshot: { snapshots.append($0) }, publishInterval: 0)
        precondition(snapshots.first!.isEmpty)
        precondition(snapshots.last!.reduce(0) { $0 + $1.bytes } == 579)
        precondition(snapshots.contains { !$0.isEmpty && $0.count < 2 })
        // A single large directory must publish while it is still being read.
        for i in 0..<256 { try Data(repeating: 1, count: 10).write(to: root.appendingPathComponent("folder/nested/test-\(i)")) }
        let token = Cancellation()
        var liveBytes: [Int64] = []
        _ = try scan(root.appendingPathComponent("folder"), token: token, progress: { _ in }, snapshot: { partial in
            let total = partial.reduce(Int64(0)) { $0 + $1.bytes }
            liveBytes.append(total)
            if total > 0 { token.cancel() }
        }, publishInterval: 0)
        precondition(token.cancelled)
        precondition(liveBytes.contains { $0 > 0 && $0 < 2683 })
        precondition(entries.count == 2)
        precondition(entries[0].bytes == 456 && entries[1].bytes == 123)
        precondition(entries[1].directory)
        precondition(entries.allSatisfy { $0.errors == 0 })
        let cancelled = Cancellation(); cancelled.cancel()
        let cancelledResult = try scan(root, token: cancelled, progress: { _ in })
        precondition(cancelledResult.isEmpty)
        do { _ = try scan(root.appendingPathComponent("missing"), token: Cancellation(), progress: { _ in }); fatalError("Missing root must fail") } catch {}
        print("Scanner: exact totals, sorting, recursive descent, symlink cycle, cancellation and missing-root tests passed")
    }
}
