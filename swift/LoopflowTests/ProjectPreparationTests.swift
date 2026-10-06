import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Project preparation")
@MainActor
struct ProjectPreparationTests {
    @Test("opening failure is retryable without changing retained planning")
    func retry() async {
        let preparation = ProjectPreparation()
        await preparation.open(wave: "a", repo: "/repo", query: RegistryQuery { _, _ in
            throw RegistryQueryError("offline")
        })
        #expect(!preparation.isPreparing)
        #expect(preparation.errorMessage == "Project preparation failed: offline")
        await preparation.open(wave: "a", repo: "/repo", query: RegistryQuery { _, _ in "{}" })
        #expect(preparation.errorMessage == nil)
        #expect(!preparation.isPreparing)
    }

    @Test("late failure from a prior opening cannot overwrite the new Wave")
    func lateResponse() async {
        let response = DelayedPreparation()
        let preparation = ProjectPreparation()
        let old = Task {
            await preparation.open(wave: "a", repo: "/repo", query: RegistryQuery { _, _ in
                await response.wait()
                throw RegistryQueryError("old failure")
            })
        }
        await response.started()
        #expect(preparation.isPreparing)
        await preparation.open(wave: "b", repo: "/repo", query: RegistryQuery { _, _ in "{}" })
        await response.finish()
        await old.value
        #expect(!preparation.isPreparing)
        #expect(preparation.errorMessage == nil)
    }

    @Test("rotation report retains destinations and unresolved Task reasons")
    func rotationPreview() async throws {
        let query = RegistryQuery(runWithInput: { _, _, _ in
            #"{"name":"October","waves":[{"wave":"a","successor_id":"next","predecessor":{"id":"before","name":"Summer work"},"successor":null,"tasks":[{"task":{"id":"issue","identifier":"A-1","name":"Keep work"},"disposition":"unresolved","reason":"Provider unavailable"}]}]}"#
        }, run: { _, _ in throw RegistryQueryError("unexpected read") })
        let result = try await query.realignProjects(name: "October", plan: "{}", preview: true, cwd: "/repo")
        #expect(result.waves.first?.predecessor?.name == "Summer work")
        #expect(result.waves.first?.successor_id == "next")
        #expect(result.waves.first?.tasks.first?.reason == "Provider unavailable")
    }
}

private actor DelayedPreparation {
    private var pending: CheckedContinuation<Void, Never>?
    private var observer: CheckedContinuation<Void, Never>?

    func wait() async {
        await withCheckedContinuation { continuation in
            pending = continuation
            observer?.resume()
            observer = nil
        }
    }

    func started() async {
        if pending != nil { return }
        await withCheckedContinuation { observer = $0 }
    }

    func finish() {
        pending?.resume()
        pending = nil
    }
}
