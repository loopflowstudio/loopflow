import Foundation

struct AnthropicClient {
    let session: URLSession
    let apiKey: String?
    let model: String

    init(
        session: URLSession = .shared,
        apiKey: String? = ProcessInfo.processInfo.environment["ANTHROPIC_API_KEY"],
        model: String = ProcessInfo.processInfo.environment["ANTHROPIC_MODEL"] ?? "claude-sonnet-4-5-20250929"
    ) {
        self.session = session
        self.apiKey = apiKey?.trimmingCharacters(in: .whitespacesAndNewlines)
        self.model = model
    }

    var hasAPIKey: Bool {
        apiKey?.isEmpty == false
    }

    func complete(message: String, system: String) async throws -> String {
        guard let apiKey, !apiKey.isEmpty else {
            throw AnthropicClientError.missingAPIKey
        }

        let url = URL(string: "https://api.anthropic.com/v1/messages")!
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue(apiKey, forHTTPHeaderField: "x-api-key")
        request.setValue("2023-06-01", forHTTPHeaderField: "anthropic-version")

        let payload = AnthropicRequest(
            model: model,
            maxTokens: 1024,
            system: system.isEmpty ? nil : system,
            messages: [AnthropicRequest.Message(role: "user", content: message)]
        )
        request.httpBody = try JSONEncoder().encode(payload)

        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else {
            throw AnthropicClientError.invalidResponse
        }

        guard http.statusCode == 200 else {
            if let decoded = try? JSONDecoder().decode(AnthropicErrorEnvelope.self, from: data) {
                throw AnthropicClientError.requestFailed(decoded.error.message)
            }
            throw AnthropicClientError.requestFailed("HTTP \(http.statusCode)")
        }

        let decoded = try JSONDecoder().decode(AnthropicResponse.self, from: data)
        let text = decoded.content
            .filter { $0.type == "text" }
            .compactMap(\.text)
            .joined(separator: "\n")
            .trimmingCharacters(in: .whitespacesAndNewlines)

        guard !text.isEmpty else {
            throw AnthropicClientError.emptyResponse
        }
        return text
    }
}

enum AnthropicClientError: LocalizedError {
    case missingAPIKey
    case invalidResponse
    case emptyResponse
    case requestFailed(String)

    var errorDescription: String? {
        switch self {
        case .missingAPIKey:
            return "ANTHROPIC_API_KEY is not set."
        case .invalidResponse:
            return "No response from Anthropic."
        case .emptyResponse:
            return "Anthropic returned an empty response."
        case .requestFailed(let message):
            return message
        }
    }
}

private struct AnthropicRequest: Encodable {
    struct Message: Encodable {
        let role: String
        let content: String
    }

    let model: String
    let maxTokens: Int
    let system: String?
    let messages: [Message]

    enum CodingKeys: String, CodingKey {
        case model
        case maxTokens = "max_tokens"
        case system
        case messages
    }
}

private struct AnthropicResponse: Decodable {
    struct Content: Decodable {
        let type: String
        let text: String?
    }

    let content: [Content]
}

private struct AnthropicErrorEnvelope: Decodable {
    struct ErrorInfo: Decodable {
        let message: String
    }

    let error: ErrorInfo
}
