# Research: active Swift fixture after SQL discovery

## System understanding
2026-09-29 · Bounded LOO-298 contribution for Jack Heart. Confirmed cwd `/Users/jack/src/loopflow.data-model-one-table-per` before source reads. Read governing guide, research skill, README, supplied Infrastructure memory and current remaining-work. Only this artifact is written; no builds/tests/providers, installed Home, Git/PM mutations or delegation.

**Observed:** CI36546252120/job109333256813 on3280c7524 records `realCLI` failing at nextReady's line251 deadline after13.928s (retained log lines1556–1557). This supplies no phase/count/gap diagnostic and does not establish slowness.
**Architecture/data flow:** `publishClient` writes a 2020 manifest and live cat receipt only. `active/reader.rs::collect` discovers/verifies the receipt; `active.rs::project` now requires `SqliteStore::input_snapshot`, records lookup failure as a gap and excludes that client. The SQL query resolves Session/input ownership before history projection; it never imports a manifest. No admitted row exists in this fixture. This source mismatch explains missing discovery without a timing hypothesis; no fresh reproduction was run.
**Import prerequisites:** `session_import::run/history/store` accepts schema1 TUI manifest, valid `run_`+UUID input, parsed native reference and nullable ancestry. Existing 32-hex IDs and `runs/00/` shard are valid (`durable.rs`); use schema1/nonempty `provider_session_id` and explicit null account. Import retains manifest `created_at=2020-01-01T00:00:00Z`; no synthetic completion or provider launch is required. A second import retains the first and adds one interactive Session. JSON mode returns success even with `failed[]`, so exit0 alone is insufficient.

## Tensions and observations
Explicit legacy import is the smallest public admission route that preserves the historical timestamp; a fresh agent launch adds provider work and today's chronology. This fixture proves automatic observation of *admitted* clients, not automatic legacy import. Keep receipt time near actual cat start (write it before import); it is distinct from historical conversation time. Current active lookup has no recency cutoff beyond epoch0.
Cleanup currently relies on throwing-path defers that terminate cats without awaiting them and does not explicitly await reader cancellation. The proposal adds awaited catch cleanup, retaining normal assertions and process-specific ownership. No guessed SQL, production manifest reader, extended deadline or relaxed assertion is introduced. Indentation inside the two added `do` blocks is deliberately left unchanged to keep the review diff small; main should format it when applying.

## Recommendations
**Unapplied, uncompiled, unexecuted patch** for the single Swift fixture below. It pins one source binary, temporary Home/database and cwd, strips all inherited LF_/LOOPFLOW_ variables, imports each authored input, and checks the report. File output avoids pipe backpressure; the import termination continuation waits for its exact child. All original first/second discovery, automatic tick, no-gap, rescan, second exit and cancellation-survivor assertions remain. Failure catches await exact cats/reader before Home deletion; no provider process is launched. Cost: one fixture-only helper plus import/cleanup. Benefit: exercise the production SQL owner while preserving this transport proof.

```diff
--- a/swift/LoopflowTests/ActiveRunsObservationTests.swift
+++ b/swift/LoopflowTests/ActiveRunsObservationTests.swift
@@ -187,15 +187,10 @@
         }
+        var observation: ActiveRunsObservation?
+        do {
         let firstID = "run_00000000000000000000000000000001"
         let secondID = "run_00000000000000000000000000000002"
-        try publishClient(client, id: firstID, home: home)
-        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
-            .deletingLastPathComponent().deletingLastPathComponent()
-        let process = LocalWaveAgentLauncher.queryProcess([
-            root.appendingPathComponent("target/debug/lf").path, "runs", "--active", "--watch", "--json",
-        ])
-        var environment = process.environment ?? [:]
-        for key in ["LF_HOME", "LF_CONTROL_HOME"] { environment[key] = home.path }
-        for key in ["LF_DB_PATH", "LF_CONTROL_DB_PATH"] { environment[key] = home.appendingPathComponent("loopflow.db").path }
-        process.environment = environment
-        let reader = try LocalActiveRunsObservation.start(process: process, configurationChanged: { false })
+        try await publishClient(client, id: firstID, home: home)
+        let process = fixtureProcess(["runs", "--active", "--watch", "--json"], home: home)
+        let reader = try LocalActiveRunsObservation.start(process: process, configurationChanged: { false })
+        observation = reader
         var iterator = reader.snapshots.makeAsyncIterator()
@@ -219,3 +214,4 @@
         }
-        try publishClient(secondClient, id: secondID, home: home)
+        do {
+        try await publishClient(secondClient, id: secondID, home: home)
         let published = try await nextReady(&iterator, count: 2)
@@ -236,2 +232,15 @@
         #expect(await clientExit.next() == 0)
+        } catch {
+            try? secondInput.fileHandleForWriting.close()
+            if secondClient.isRunning { secondClient.terminate() }
+            _ = await secondExit.next()
+            throw error
+        }
+        } catch {
+            await observation?.cancel()
+            try? clientInput.fileHandleForWriting.close()
+            if client.isRunning { client.terminate() }
+            _ = await clientExit.next()
+            throw error
+        }
     }
@@ -257,3 +266,3 @@
 
-    private func publishClient(_ client: Process, id: String, home: URL) throws {
+    private func publishClient(_ client: Process, id: String, home: URL) async throws {
         let directory = home.appendingPathComponent("runs/00/\(id)")
@@ -272,2 +281,36 @@
             to: directory.appendingPathComponent("provider-clients/\(client.processIdentifier).json"), options: .atomic)
+        let native: [String: Any] = ["schema_version": 1, "provider_session_id": "fixture-\(id)", "account_id": NSNull()]
+        try JSONSerialization.data(withJSONObject: native).write(
+            to: directory.appendingPathComponent("provider-session.json"), options: .atomic)
+        let output = home.appendingPathComponent("import-\(id).json")
+        try Data().write(to: output)
+        let handle = try FileHandle(forWritingTo: output)
+        defer { try? handle.close() }
+        let command = fixtureProcess(["session", "import", "--json"], home: home)
+        command.standardOutput = handle
+        let status: Int32 = try await withCheckedThrowingContinuation { continuation in
+            command.terminationHandler = { continuation.resume(returning: $0.terminationStatus) }
+            do { try command.run() }
+            catch { command.terminationHandler = nil; continuation.resume(throwing: error) }
+        }
+        try #require(status == 0)
+        let report = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: output)) as? [String: Any])
+        let failures = try #require(report["failed"] as? [[String: Any]])
+        try #require(failures.isEmpty, "Import failed: \(failures)")
+        try #require(report["interactive"] as? Int == 1)
+    }
+
+    private func fixtureProcess(_ arguments: [String], home: URL) -> Process {
+        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
+            .deletingLastPathComponent().deletingLastPathComponent()
+        let binary = root.appendingPathComponent("target/debug/lf").path
+        let process = LocalWaveAgentLauncher.queryProcess([binary] + arguments, cwd: home.path)
+        var environment = (process.environment ?? [:]).filter {
+            !$0.key.hasPrefix("LF_") && !$0.key.hasPrefix("LOOPFLOW_")
+        }
+        for key in ["LF_BIN", "LF_CONTROL_BIN"] { environment[key] = binary }
+        for key in ["LF_HOME", "LF_CONTROL_HOME"] { environment[key] = home.path }
+        for key in ["LF_DB_PATH", "LF_CONTROL_DB_PATH"] { environment[key] = home.appendingPathComponent("loopflow.db").path }
+        process.environment = environment
+        return process
     }
```

## Open questions / proof for main
No latency or process-settlement pass is claimed. The retained log does not reveal which nextReady invocation failed; source establishes the admission defect independently. Main's concurrent importer work can alter prerequisites; recheck hashes before applying. The import subprocess's nonthrowing termination callback is awaited, not Foundation `waitUntilExit()` after suspension (TESTING.md). The existing overall proof timeout still bounds unexpected import hangs; this patch adds no timeout/retry policy.
Build the current source CLI in main's serialized slot, then run only this proof through the existing isolated runner (commands **not run here**):
```sh
uv run python .lf/tmp/cut-i/run.py active-swift-fixture-build cargo build -p loopflow --bin lf -j 4
uv run python .lf/tmp/cut-i/run.py active-swift-fixture swift test --package-path swift --no-parallel -Xswiftc -gnone --filter 'ActiveRunsObservationTests/realCLI'
```
Require an actually executed realCLI test, unchanged 12-second nextReady deadline, both observed IDs/no gaps and all exit/survivor assertions. Preserve import diagnostics if it fails; do not infer performance from another deadline. This is source-CLI/synthetic cat transport evidence, not configured provider/Desktop acceptance or completed Run removal.

Inspected SHA-256 (read-only source/log identity, not validation):
```text
e6b21711a2b078964801ca8338c7062e13465b9918b0e0ba16e66b22570cb1f5  swift/LoopflowTests/ActiveRunsObservationTests.swift
e08143adce8ead272872c05f055c2f5d645593b48d7cba64b577754f916bd578  rust/loopflow/src/run_record/active.rs
b5e550640192e4610619fde491bc1835c6b134b9df0a0a6fb0a7777ef8671ffe  rust/loopflow/src/run_record/active/reader.rs
112a4a8c07b71759e98aa621b9a9e33d1f715f4d66eb096cd5cbe41a16c96ee6  rust/loopflow/src/ops/session_import.rs
3799c32b94d44e56f87abc14993880b2c80c7a3d7c07a372d028d67774afa4b2  rust/loopflow/src/store/sqlite/sessions.rs
8a7fe75bf65bdb27ad6cf5a25e7e0ad7ca7d0eb8c1046693a29ea08635347e26  rust/loopflow/src/run_record.rs
34dba18aa9b183866d6b0d09b38c8a6452760c8312bc5053102e243f8ac9fe4e  rust/loopflow/src/durable.rs
6220a2405864a46fabac9206e9cf4bac0f8c9e2ad3d8e0e31eecdfcd342799d1  rust/loopflow/src/lf/commands/session.rs
07901e42542b301bc8010c9a519ea6d0435cd597c1574284c73387d27cb11882  swift/LoopflowMac/Services/LocalActiveRunsObservation.swift
ecf577cb1eaa3690fad0cee6d8e01b6e5abb013bc3339ac8e4c8b3050955ab77  swift/LoopflowMac/MacLocalWaveAgentLauncher.swift
ad969bf676ad18702b8150c1ece6d68c0b09a9bc821791ed43657f3ad2cc9241  TESTING.md
7e50da8780af323bee004f7a84b8bd340319750167d788708b0d986c61e59f39  .lf/tmp/cut-i/ci-3280c7524-swift.log
```
