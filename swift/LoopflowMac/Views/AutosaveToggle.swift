import Loopflow
import SwiftUI

struct AutosaveToggle: View {
    let notices: NoticeCenter
    private let defaults: UserDefaults
    @AppStorage private var savedAutosave: Bool?
    private let undoManager: UndoManager?

    init(notices: NoticeCenter, undoManager: UndoManager?, defaults: UserDefaults = .standard) {
        self.notices = notices
        self.undoManager = undoManager
        self.defaults = defaults
        _savedAutosave = AppStorage(UserDefaults.taskFileAutosaveKey, store: defaults)
    }

    var body: some View {
        Toggle(SettingsMatch.autosave.title, isOn: Binding(
            get: { savedAutosave ?? true },
            set: { saveAutosave($0) }
        ))
        .toggleStyle(.switch)
    }

    private func saveAutosave(_ enabled: Bool) {
        let previous = savedAutosave
        savedAutosave = enabled
        let undo = Notice.Action.undo(SettingsMatch.autosave.title, manager: undoManager, target: defaults) { [weak notices] defaults in
            guard defaults.object(forKey: UserDefaults.taskFileAutosaveKey) as? Bool == enabled else { return }
            defaults.set(previous, forKey: UserDefaults.taskFileAutosaveKey)
            notices?.clear(key: UserDefaults.taskFileAutosaveKey)
        }
        notices.post(Notice(enabled ? "Task file autosave on" : "Task file autosave off",
                            key: UserDefaults.taskFileAutosaveKey, action: undo))
    }
}
