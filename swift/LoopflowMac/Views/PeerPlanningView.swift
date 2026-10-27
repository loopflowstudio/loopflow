import Loopflow
import SwiftUI

/// Displays the common status without turning a receipt into convergence.
struct PeerPlanningView: View {
    let reading: WorkReading<[PeerPlanningStatus]>

    @Environment(\.palette) private var palette

    var body: some View {
        DisclosureGroup("Git planning") {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                if let error = reading.errorMessage {
                    Text("Sync status unavailable: \(error)")
                    if reading.value != nil { Text("Showing the last sync status") }
                }
                if let destinations = reading.value {
                    if !destinations.contains(where: \.active) {
                        Text("No active Git planning destination")
                    }
                    ForEach(destinations) { destination in
                        status(destination)
                    }
                    Text("Fetched, imported and published show separate steps of synchronization.")
                } else if reading.isLoading {
                    Text("Reading sync status…")
                }
            }
            .font(Typography.body(12))
            .foregroundStyle(palette.textSecondary)
            .textSelection(.enabled)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .accessibilityIdentifier("peer-planning")
    }

    private func status(_ destination: PeerPlanningStatus) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text(destination.reference)
            Text("Destination: \(destination.id)")
            if destination.active { Text("Configured for this repository") }
            Text("\(destination.selectedRecords) planning records")
            Text(destination.pendingLocal ? "Local changes pending" : "No pending local changes")
            Text("Publication: \(destination.publicationState ?? "not attempted") (\(destination.publicationRevision ?? "none"))")
            Text("Fetched: \(destination.fetchedRevision ?? "none")")
            Text("Retained import: \(destination.importedRevision ?? "none")")
            if let error = destination.acquisitionError { Text(error) }
            if let error = destination.publicationError { Text(error) }
            ForEach(destination.conflicts.indices, id: \.self) { index in
                let conflict = destination.conflicts[index]
                Text("Held \(conflict.target): \(conflict.reason)")
            }
            Text("Inspection changes nothing. `lf planning journal` lists retained values; copy one into an ordinary edit to save it again.")
        }
        .accessibilityIdentifier("peer-planning-\(destination.id)")
    }
}
