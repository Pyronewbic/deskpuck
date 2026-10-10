import DeskpuckFFI
import Foundation

/// Asks github.com for the latest-release redirect without following it; the Rust
/// core decides whether its Location names a newer version.
enum UpdateChecker {
    static let releasesURL = URL(string: DP_LATEST_RELEASE_URL)!

    static var disabledByEnvironment: Bool {
        !(ProcessInfo.processInfo.environment[DP_UPDATE_DISABLE_ENV] ?? "").isEmpty
    }

    static func newerVersion(than current: String) async -> String? {
        var request = URLRequest(url: releasesURL, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 10)
        request.httpMethod = "HEAD"
        request.setValue("Deskpuck/\(current)", forHTTPHeaderField: "User-Agent")
        let session = URLSession(configuration: .ephemeral, delegate: NoRedirects(), delegateQueue: nil)
        defer { session.finishTasksAndInvalidate() }
        guard let (_, response) = try? await session.data(for: request),
              let http = response as? HTTPURLResponse, http.statusCode == 302,
              let location = http.value(forHTTPHeaderField: "Location")
        else { return nil }
        return Core.newerVersion(location: location, current: current)
    }
}

/// Refusing the redirect makes the 302 itself the response, Location header included.
private final class NoRedirects: NSObject, URLSessionTaskDelegate {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest) async -> URLRequest? {
        nil
    }
}
