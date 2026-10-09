import Loopflow
import SwiftUI

/// Displays the common status without turning a receipt into convergence.
struct PeerPlanningView: View {
    let reading: WorkReading<[PeerPlanningStatus]>

    var body: some View {
        DisclosureGroup("Git planning") {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                if let error = reading.errorMessage {
                    Text("Sync status unavailable: \(error)")
                    if reading.value != nil { Text("Showing the last sync status") }
                }
                if let destinations = reading.value {
                    if !destinations.contains(where: \.active) {
                        Text("Future root Waves: local")
                    }
                    ForEach(destinations) { destination in
                        status(destination)
                    }
                    Text("Import retention is not publication or convergence. Linear delivery is separate.")
                } else if reading.isLoading {
                    Text("Reading sync status…")
                }
            }
            .font(Typography.body(12))
            .foregroundStyle(.secondary)
            .textSelection(.enabled)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .accessibilityIdentifier("peer-planning")
    }

    private func status(_ destination: PeerPlanningStatus) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text(destination.reference)
            Text("Destination: \(destination.id)")
            if destination.active { Text("Selected for future root Waves") }
            Text("\(destination.selectedRecords) selected records")
            switch destination.pendingLocal {
            case true?: Text("Local changes pending")
            case false?: Text("No additional eligible local changes")
            case nil: Text("Local changes unknown")
            }
            Text("Publication: \(destination.publicationState ?? "not attempted") (\(destination.publicationRevision ?? "none"))")
            Text("Fetched: \(destination.fetchedRevision ?? "none")")
            Text("Retained import: \(destination.importedRevision ?? "none")")
            if let error = destination.localError { Text(error) }
            if let error = destination.acquisitionError { Text(error) }
            if let error = destination.publicationError { Text(error) }
            ForEach(destination.conflicts.indices, id: \.self) { index in
                let conflict = destination.conflicts[index]
                Text("Held \(conflict.object.kind) \(conflict.object.id): \(conflict.reason)")
            }
        }
        .accessibilityIdentifier("peer-planning-\(destination.id)")
    }
}
