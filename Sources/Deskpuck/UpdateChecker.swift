import DeskpuckFFI
import Foundation
import Security

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

/// One-click update: the Rust core names and verifies the downloads, and the new app must be signed
/// like this one. Any failure leaves the running app as it was.
enum Installer {
    enum Failure: Error {
        case download, verify, install
    }

    /// Replacing the bundle needs its folder writable by this user.
    static var canInstallInPlace: Bool {
        FileManager.default.isWritableFile(atPath: Bundle.main.bundleURL.deletingLastPathComponent().path)
    }

    /// The new app, verified and unpacked beside the running one (same volume, so the
    /// swap is a rename), plus the folder to delete afterwards.
    static func prepare(version: String, current: String) async throws -> (app: URL, folder: URL) {
        guard let plan = Core.releasePlan(version: version, current: current) else { throw Failure.verify }
        let fileManager = FileManager.default
        let folder: URL
        do {
            folder = try fileManager.url(for: .itemReplacementDirectory, in: .userDomainMask,
                                         appropriateFor: Bundle.main.bundleURL, create: true)
        } catch {
            throw Failure.install
        }
        do {
            try await download(plan, into: folder, current: current)
            if let refusal = Core.verifyRelease(in: folder, version: version, current: current) {
                NSLog("Deskpuck update: %@", refusal)
                throw Failure.verify
            }
            let app = try unpack(folder.appendingPathComponent(plan.package), into: folder)
            guard signedLikeThisApp(app), bundleVersion(app) == plan.version else { throw Failure.verify }
            return (app, folder)
        } catch {
            try? fileManager.removeItem(at: folder)
            throw error
        }
    }

    /// Moves the running app aside and the new one into its place; returns where the old
    /// one went, for `restore` or deletion.
    static func swap(in app: URL) throws -> URL {
        let fileManager = FileManager.default
        let installed = Bundle.main.bundleURL
        let aside = installed.deletingLastPathComponent()
            .appendingPathComponent(".Deskpuck-previous-\(UUID().uuidString).app")
        do {
            try fileManager.moveItem(at: installed, to: aside)
        } catch {
            throw Failure.install
        }
        do {
            try fileManager.moveItem(at: app, to: installed)
        } catch {
            try? fileManager.moveItem(at: aside, to: installed)
            throw Failure.install
        }
        return aside
    }

    /// Puts the old app back after a swap whose new app did not start.
    static func restore(_ aside: URL) {
        let fileManager = FileManager.default
        let installed = Bundle.main.bundleURL
        let failed = aside.deletingLastPathComponent()
            .appendingPathComponent(".Deskpuck-failed-\(UUID().uuidString).app")
        guard (try? fileManager.moveItem(at: installed, to: failed)) != nil else { return }
        if (try? fileManager.moveItem(at: aside, to: installed)) != nil {
            try? fileManager.removeItem(at: failed)
        } else {
            try? fileManager.moveItem(at: failed, to: installed)
        }
    }

    private static func download(_ plan: ReleasePlan, into folder: URL, current: String) async throws {
        for file in plan.downloads {
            guard let url = URL(string: file.url) else { throw Failure.download }
            let path = folder.appendingPathComponent(file.name)
            guard FileManager.default.createFile(atPath: path.path, contents: nil),
                  let handle = try? FileHandle(forWritingTo: path)
            else { throw Failure.download }
            var request = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData)
            request.setValue("Deskpuck/\(current)", forHTTPHeaderField: "User-Agent")
            let fetched = await CappedDownload(into: handle, maxBytes: file.maxBytes).run(request)
            try? handle.close()
            guard fetched else { throw Failure.download }
        }
    }

    private static func unpack(_ zip: URL, into folder: URL) throws -> URL {
        let unpacked = folder.appendingPathComponent("unpacked")
        let ditto = Process()
        ditto.executableURL = URL(fileURLWithPath: "/usr/bin/ditto")
        ditto.arguments = ["-x", "-k", zip.path, unpacked.path]
        do {
            try ditto.run()
        } catch {
            throw Failure.install
        }
        ditto.waitUntilExit()
        guard ditto.terminationStatus == 0 else { throw Failure.install }
        return unpacked.appendingPathComponent("Deskpuck.app")
    }

    /// Valid, and meeting this app's own designated requirement: the same identifier and
    /// certificate, which is also what keeps the Accessibility permission across the swap.
    private static func signedLikeThisApp(_ app: URL) -> Bool {
        var me: SecCode?
        var myStatic: SecStaticCode?
        var requirement: SecRequirement?
        var candidate: SecStaticCode?
        guard SecCodeCopySelf([], &me) == errSecSuccess, let me,
              SecCodeCopyStaticCode(me, [], &myStatic) == errSecSuccess, let myStatic,
              SecCodeCopyDesignatedRequirement(myStatic, [], &requirement) == errSecSuccess, let requirement,
              SecStaticCodeCreateWithPath(app as CFURL, [], &candidate) == errSecSuccess, let candidate
        else { return false }
        let flags = SecCSFlags(rawValue: kSecCSStrictValidate | kSecCSCheckAllArchitectures | kSecCSCheckNestedCode)
        return SecStaticCodeCheckValidity(candidate, flags, requirement) == errSecSuccess
    }

    private static func bundleVersion(_ app: URL) -> String? {
        Bundle(url: app)?.infoDictionary?["CFBundleShortVersionString"] as? String
    }
}

/// One download streamed to a file and cancelled past its cap: async download(for:delegate:) reports
/// no progress, so it could not be capped. Redirects go only where the core allows.
private final class CappedDownload: NSObject, URLSessionDataDelegate {
    private let handle: FileHandle
    private let maxBytes: Int64
    private var written: Int64 = 0
    private var failed = false
    private var continuation: CheckedContinuation<Bool, Never>?

    init(into handle: FileHandle, maxBytes: Int64) {
        self.handle = handle
        self.maxBytes = maxBytes
    }

    /// True only for a complete 200 response within the cap.
    func run(_ request: URLRequest) async -> Bool {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 60
        configuration.timeoutIntervalForResource = 30 * 60
        let queue = OperationQueue()
        queue.maxConcurrentOperationCount = 1
        let session = URLSession(configuration: configuration, delegate: self, delegateQueue: queue)
        defer { session.finishTasksAndInvalidate() }
        return await withCheckedContinuation { continuation in
            queue.addOperation {
                self.continuation = continuation
                session.dataTask(with: request).resume()
            }
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        guard let url = request.url, Core.redirectAllowed(url) else { return completionHandler(nil) }
        completionHandler(request)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse,
                    completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        let ok = (response as? HTTPURLResponse)?.statusCode == 200 && response.expectedContentLength <= maxBytes
        failed = failed || !ok
        completionHandler(ok ? .allow : .cancel)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        written += Int64(data.count)
        guard written <= maxBytes, (try? handle.write(contentsOf: data)) != nil else {
            failed = true
            dataTask.cancel()
            return
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        let ok = error == nil && !failed && (task.response as? HTTPURLResponse)?.statusCode == 200
        continuation?.resume(returning: ok)
        continuation = nil
    }
}
