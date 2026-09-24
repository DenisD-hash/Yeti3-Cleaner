import Foundation

// Every filesystem result enters this buffer immediately. At most one UI task is
// pending, so a fast disk cannot flood the main queue with obsolete redraws.
// There is no timer or minimum batch size before displaying received data.
final class LiveScan: @unchecked Sendable {
    private let lock = NSLock()
    private var entries: [String: Entry] = [:]
    private var count = 0
    private var pending = false
    private let display: @MainActor ([Entry], Int) -> Void
    init(display: @escaping @MainActor ([Entry], Int) -> Void) { self.display = display }
    func receive(_ entry: Entry, count: Int) {
        lock.lock()
        entries[entry.id] = entry; self.count = count
        let schedule = !pending; pending = true
        lock.unlock()
        if schedule {
            DispatchQueue.main.async {
                self.lock.lock()
                let current = Array(self.entries.values); let count = self.count
                self.pending = false
                self.lock.unlock()
                self.display(current.sorted { $0.bytes > $1.bytes }, count)
            }
        }
    }
    func snapshot() -> [Entry] {
        lock.lock(); defer { lock.unlock() }
        return Array(entries.values)
    }
}
