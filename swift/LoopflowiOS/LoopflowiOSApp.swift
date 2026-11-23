import Loopflow
import Observation
import SwiftUI
import UIKit

/// UIKit owns event dispatch; the screens remain SwiftUI.
@main
enum LoopflowiOSApp {
    static func main() {
        UIApplicationMain(CommandLine.argc, CommandLine.unsafeArgv,
                          NSStringFromClass(InputApplication.self), NSStringFromClass(MobileAppDelegate.self))
    }
}

final class MobileAppDelegate: UIResponder, UIApplicationDelegate {
    func application(_ application: UIApplication, didFinishLaunchingWithOptions options: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        bootstrapLoopflowApp()
        return true
    }

    func application(_ application: UIApplication, configurationForConnecting session: UISceneSession,
                     options: UIScene.ConnectionOptions) -> UISceneConfiguration {
        let configuration = UISceneConfiguration(name: nil, sessionRole: session.role)
        configuration.delegateClass = MobileSceneDelegate.self
        return configuration
    }
}

@Observable @MainActor
private final class SceneActivity {
    var phase = ScenePhase.inactive
}

@MainActor
final class MobileSceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    private let activity = SceneActivity()

    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options: UIScene.ConnectionOptions) {
        guard let scene = scene as? UIWindowScene else { return }
        let window = UIWindow(windowScene: scene)
        window.rootViewController = UIHostingController(rootView: MobileRoot(activity: activity))
        self.window = window
        window.makeKeyAndVisible()
    }

    func sceneDidBecomeActive(_ scene: UIScene) { activity.phase = .active }
    func sceneWillResignActive(_ scene: UIScene) { activity.phase = .inactive }
    func sceneDidEnterBackground(_ scene: UIScene) {
        activity.phase = .background
        Task { await InteractionFrame.disableCapture() }
    }
}

private struct MobileRoot: View {
    let activity: SceneActivity
    @Environment(\.colorScheme) private var colorScheme
    var body: some View {
        MachineConnectionView()
            .environment(\.scenePhase, activity.phase)
            .environment(\.palette, (colorScheme == .dark ? ThemeDefinition.dark : .light).palette)
    }
}
