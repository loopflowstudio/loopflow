import SwiftUI

public struct AuthServiceKey: EnvironmentKey {
    public static let defaultValue = AuthService()
}

public extension EnvironmentValues {
    var authService: AuthService {
        get { self[AuthServiceKey.self] }
        set { self[AuthServiceKey.self] = newValue }
    }
}
