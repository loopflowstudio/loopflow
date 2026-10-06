import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Project activation")
@MainActor
struct ProjectActivationTests {
    @Test("activation failure belongs to its Wave and retry preserves planning")
    func retry() async throws {
        let path = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(contentsOf: path))
        let result = ActivationResult()
        let model = PodiumModel(query: RegistryQuery { _, _ in
            if await result.offline { throw RegistryQueryError("offline") }
            return "{}"
        })
        model.applyFixture(roadmap: .available(roadmap), waves: .available([]),
            processActivity: .loading, workActivity: .loading, repos: [])
        await model.activateProject(id: "a", name: "a", repo: "/repo")?.value
        #expect(model.projectCommandErrors["a"] == "offline")
        #expect(model.projectCommandErrors["b"] == nil)
        #expect(model.roadmap.value == roadmap)
        await result.recover()
        await model.activateProject(id: "a", name: "a", repo: "/repo")?.value
        #expect(model.projectCommandErrors["a"] == nil)
        #expect(model.roadmap.value == roadmap)
    }

    @Test("leaving a Wave does not cancel its activation or replace another Wave's feedback")
    func lateResponse() async {
        let response = DelayedPreparation()
        let model = PodiumModel(query: RegistryQuery { args, _ in
            if args.contains("a") {
                await response.wait()
                throw RegistryQueryError("a failed")
            }
            return "{}"
        })
        let old = model.activateProject(id: "a", name: "a", repo: "/repo")
        await response.started()
        model.select(nil)
        await model.activateProject(id: "b", name: "b", repo: "/repo")?.value
        await response.finish()
        await old?.value
        #expect(model.projectCommandErrors["a"] == "a failed")
        #expect(model.projectCommandErrors["b"] == nil)
    }

    @Test("a Project without a default Flow renders no unavailable template")
    func absentFlow() throws {
        let path = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        var data = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: path)) as? [String: Any])
        var waves = try #require(data["waves"] as? [[String: Any]])
        var projects = try #require(waves[0]["projects"] as? [String: Any])
        var items = try #require(projects["items"] as? [[String: Any]])
        for index in items.indices { items[index]["flow"] = "" }
        projects["items"] = items; waves[0]["projects"] = projects; data["waves"] = waves
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: data))
        let model = PodiumModel(query: RegistryQuery { _, _ in "{}" })
        model.applyFixture(roadmap: .available(roadmap), waves: .available([]),
            processActivity: .loading, workActivity: .loading, repos: [])
        model.select(.wave(id: roadmap.waves[0].wave.id))
        let view = try WorkSurfaceView(model: model).inspect()
        #expect(view.findAll(ViewType.Text.self) {
            let text = (try? $0.string()) ?? ""
            return text.hasPrefix("Flow ·") || text == "Flow template unavailable"
        }.isEmpty)
        #expect(try view.find(text: "Current KRs").string() == "Current KRs")
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

private actor ActivationResult {
    var offline = true
    func recover() { offline = false }
}
