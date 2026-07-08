import Foundation
import Testing
@testable import Loopflow

@Suite("Catalog")
struct CatalogTests {
    @Test("catalog response round-trips nested flow items")
    func catalogResponseRoundTripsNestedFlowItems() throws {
        let response = try JSONDecoder().decode(
            CatalogResponse.self,
            from: Data(sampleCatalogResponse.utf8)
        )

        let encoded = try JSONEncoder().encode(response)
        let roundTripped = try JSONDecoder().decode(CatalogResponse.self, from: encoded)

        #expect(roundTripped.result == response.result)
        #expect(roundTripped.result.flowsByName["build"]?.category == "Build")
        #expect(roundTripped.result.stepsByName["gate"]?.source == .repo)
    }

    @Test("catalog computes direct parents")
    func catalogComputesDirectParents() throws {
        let catalog = try JSONDecoder().decode(
            CatalogResponse.self,
            from: Data(sampleCatalogResponse.utf8)
        ).result

        #expect(catalog.directParents(of: "gate").map(\.name) == ["build", "code"])
        #expect(catalog.directParents(of: "code").map(\.name) == ["build"])
    }
}

private let sampleCatalogResponse = """
{
  "ok": true,
  "result": {
    "flows": [
      {
        "name": "build",
        "category": "Build",
        "source": "builtin",
        "items": [
          {"type": "FlowRef", "data": "code"},
          {
            "type": "Loop",
            "data": {
              "steps": [
                {"type": "Step", "data": {"name": "implement", "interactive": false}}
              ],
              "exit": {
                "router": "gate",
                "paths": {
                  "done": {"description": "Ship it"}
                }
              }
            }
          }
        ]
      },
      {
        "name": "code",
        "category": "Build",
        "source": "builtin",
        "items": [
          {"type": "Step", "data": {"name": "implement", "interactive": false}},
          {"type": "Step", "data": {"name": "gate", "interactive": false}}
        ]
      }
    ],
    "steps": [
      {
        "name": "implement",
        "category": "Build",
        "source": "builtin",
        "description": "Build from a design doc",
        "interactive": false
      },
      {
        "name": "gate",
        "category": "Build",
        "source": "repo",
        "description": "Ship-ready code and reviewer-friendly docs",
        "interactive": false
      }
    ]
  }
}
"""
