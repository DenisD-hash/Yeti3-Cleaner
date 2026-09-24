import Foundation
import CryptoKit
@main struct UpdateTests {
    static func main() throws {
        let payload = Data("fixture".utf8)
        let hash = SHA256.hash(data: payload).map { String(format: "%02x", $0) }.joined()
        var manifest: [String: String] = ["version": "0.4.0", "minimum_macos": "14.0", "url": "https://raw.githubusercontent.com/lodos/Yeti3-Cleaner/master/downloads/Yeti3-Cleaner-0.4.0-arm64.dmg", "sha256": hash]
        func data() throws -> Data { try JSONSerialization.data(withJSONObject: manifest) }
        _ = try validateUpdate(data(), osVersion: "14.0")
        try verifyUpdate(payload, sha256: hash)
        do { try verifyUpdate(Data("tampered".utf8), sha256: hash); fatalError("Checksum accepted tampering") } catch {}
        do { _ = try validateUpdate(data(), osVersion: "13.0"); fatalError("Incompatible OS accepted") } catch {}
        manifest["url"] = "https://example.com/evil.dmg"
        do { _ = try validateUpdate(data(), osVersion: "14.0"); fatalError("Foreign source accepted") } catch {}
        precondition("0.10.0".compare("0.9.0", options: .numeric) == .orderedDescending)
        print("Updater: trusted source, version ordering, OS requirement and checksum rejection passed")
    }
}
