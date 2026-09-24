import Foundation
import CryptoKit
struct ReleaseVersion: Comparable {
    let numbers: [Int]
    let candidate: Int?
    init?(_ text: String) {
        let halves = text.components(separatedBy: "-rc.")
        guard halves.count <= 2 else { return nil }
        let core = halves[0].components(separatedBy: ".")
        guard core.count == 3, core.allSatisfy({ !$0.isEmpty && $0.allSatisfy(\.isNumber) && Int($0) != nil }) else { return nil }
        numbers = core.compactMap(Int.init)
        if halves.count == 2 {
            guard let n = Int(halves[1]), n > 0, halves[1].allSatisfy(\.isNumber) else { return nil }
            candidate = n
        } else { candidate = nil }
    }
    static func < (a: Self, b: Self) -> Bool {
        if a.numbers != b.numbers { return a.numbers.lexicographicallyPrecedes(b.numbers) }
        switch (a.candidate, b.candidate) {
        case (.some(let x), .some(let y)): return x < y
        case (.some, .none): return true
        default: return false
        }
    }
}
struct UpdateInfo: Codable {
    let version: String; let url: URL; let sha256: String; let minimum_macos: String
    let prerelease: Bool?; let architecture: String?
}
func validateUpdate(_ data: Data, osVersion: String, allowPrerelease: Bool = false) throws -> UpdateInfo {
    let release = try JSONDecoder().decode(UpdateInfo.self, from: data)
    let osParts = release.minimum_macos.split(separator: ".", omittingEmptySubsequences: false)
    let arch = release.architecture ?? "arm64"
    #if arch(arm64)
    let hostArchitecture = "arm64"
    #else
    let hostArchitecture = "x86_64"
    #endif
    guard arch == "universal" || arch == hostArchitecture else {
        throw NSError(domain: "Yeti3", code: 4, userInfo: [NSLocalizedDescriptionKey: "Эта сборка не подходит для процессора этого Mac."])
    }
    guard let version = ReleaseVersion(release.version),
          ["arm64", "x86_64", "universal"].contains(arch),
          (1...3).contains(osParts.count), osParts.allSatisfy({ !$0.isEmpty && $0.allSatisfy(\.isNumber) }),
          release.url.scheme == "https", release.url.host == "raw.githubusercontent.com",
          release.url.path == "/lodos/Yeti3-Cleaner/master/downloads/Yeti3-Cleaner-\(release.version)-\(arch).dmg",
          release.url.query == nil, release.url.fragment == nil,
          release.sha256.count == 64, release.sha256.allSatisfy(\.isHexDigit),
          (release.prerelease ?? false) == (version.candidate != nil) else { throw URLError(.unsupportedURL) }
    if version.candidate != nil && !allowPrerelease {
        throw NSError(domain: "Yeti3", code: 3, userInfo: [NSLocalizedDescriptionKey: "Предрелиз доступен только при включённом канале предварительных версий."])
    }
    guard release.minimum_macos.compare(osVersion, options: .numeric) != .orderedDescending else {
        throw NSError(domain: "Yeti3", code: 2, userInfo: [NSLocalizedDescriptionKey: "Новая версия требует macOS \(release.minimum_macos) или новее."])
    }
    return release
}
func verifyUpdate(_ data: Data, sha256: String) throws {
    let hash = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    guard hash == sha256.lowercased() else { throw NSError(domain: "Yeti3", code: 1, userInfo: [NSLocalizedDescriptionKey: "Контрольная сумма не совпала. Установка отменена."]) }
}
