import Foundation
import CryptoKit
@main struct UpdateTests {
    static func main() throws {
        let payload = Data("fixture".utf8)
        let hash = SHA256.hash(data: payload).map { String(format: "%02x", $0) }.joined()
        var manifest: [String: Any] = ["version": "0.4.1-rc.1", "minimum_macos": "14.0", "architecture": "universal", "prerelease": true, "url": "https://raw.githubusercontent.com/lodos/Yeti3-Cleaner/master/downloads/Yeti3-Cleaner-0.4.1-rc.1-universal.dmg", "sha256": hash]
        func data() throws -> Data { try JSONSerialization.data(withJSONObject: manifest) }
        _ = try validateUpdate(data(), osVersion: "14.0", allowPrerelease: true)
        try verifyUpdate(payload, sha256: hash)
        do { _ = try validateUpdate(data(), osVersion: "14.0"); fatalError("Stable channel accepted prerelease") } catch {}
        do { try verifyUpdate(Data("tampered".utf8), sha256: hash); fatalError("Checksum accepted tampering") } catch {}
        do { _ = try validateUpdate(data(), osVersion: "13.0", allowPrerelease: true); fatalError("Incompatible OS accepted") } catch {}
        manifest["url"] = "https://example.com/evil.dmg"
        do { _ = try validateUpdate(data(), osVersion: "14.0", allowPrerelease: true); fatalError("Foreign source accepted") } catch {}
        precondition(ReleaseVersion("0.10.0")! > ReleaseVersion("0.9.0")!)
        precondition(ReleaseVersion("0.4.1")! > ReleaseVersion("0.4.1-rc.9")!)
        precondition(ReleaseVersion("0.4.1-rc.10")! > ReleaseVersion("0.4.1-rc.2")!)
        print("Updater: channels, RC/stable ordering, trusted source, OS requirement and checksum rejection passed")
    }
}
