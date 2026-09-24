import Foundation
import CryptoKit
struct UpdateInfo: Codable { let version: String; let url: URL; let sha256: String; let minimum_macos: String }
func validateUpdate(_ data: Data, osVersion: String) throws -> UpdateInfo {
    let release = try JSONDecoder().decode(UpdateInfo.self, from: data)
    let parts = release.version.split(separator: ".", omittingEmptySubsequences: false)
    let osParts = release.minimum_macos.split(separator: ".", omittingEmptySubsequences: false)
    guard parts.count == 3, parts.allSatisfy({ !$0.isEmpty && $0.allSatisfy(\.isNumber) }),
          (1...3).contains(osParts.count), osParts.allSatisfy({ !$0.isEmpty && $0.allSatisfy(\.isNumber) }),
          release.url.scheme == "https", release.url.host == "raw.githubusercontent.com",
          release.url.path == "/lodos/Yeti3-Cleaner/master/downloads/Yeti3-Cleaner-\(release.version)-arm64.dmg",
          release.url.query == nil, release.url.fragment == nil,
          release.sha256.count == 64, release.sha256.allSatisfy(\.isHexDigit) else { throw URLError(.unsupportedURL) }
    guard release.minimum_macos.compare(osVersion, options: .numeric) != .orderedDescending else {
        throw NSError(domain: "Yeti3", code: 2, userInfo: [NSLocalizedDescriptionKey: "Новая версия требует macOS \(release.minimum_macos) или новее."])
    }
    return release
}
func verifyUpdate(_ data: Data, sha256: String) throws {
    let hash = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    guard hash == sha256.lowercased() else { throw NSError(domain: "Yeti3", code: 1, userInfo: [NSLocalizedDescriptionKey: "Контрольная сумма не совпала. Установка отменена."]) }
}
