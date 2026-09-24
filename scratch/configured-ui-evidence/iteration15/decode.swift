import Foundation

@main
struct VerifySessionProjection {
    static func main() throws {
        let directory = URL(fileURLWithPath: CommandLine.arguments[1])
        let records = try JSONDecoder().decode([SessionRecord].self, from: Data(contentsOf: directory.appendingPathComponent("sessions.json")))
        let cli = try String(contentsOf: directory.appendingPathComponent("sessions.txt"), encoding: .utf8)
        for record in records {
            precondition(cli.contains(record.workPath ?? "Repository"))
            for action in record.actions {
                precondition(cli.contains("\(action.label) — \(action.unavailableReason ?? action.help)"))
            }
        }
        let task = records.first { $0.work == .task(id: "task_2aa71a7e36fe416d8a721e2b2f7c54e7") }
        precondition(task?.workPath == "product / Desktop / LOO-291")
        let roundTrip = try JSONDecoder().decode([SessionRecord].self, from: JSONEncoder().encode(records))
        precondition(roundTrip == records)
        print("PASS: \(records.count) configured Session records decode and round-trip through production Swift DTOs; CLI labels/reasons agree; exact Task Work path agrees.")
    }
}
