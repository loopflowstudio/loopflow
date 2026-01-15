// Generates random worktree names from magical and musical word pairs.

import Foundation

enum NameGenerator {
    static let magical = [
        "aurora", "cascade", "crystal", "drift", "echo", "ember",
        "fern", "flume", "frost", "glade", "grove", "haze",
        "ivy", "jade", "luna", "mist", "nova", "opal",
        "petal", "prism", "rain", "ripple", "sage", "shade",
        "spark", "star", "stone", "storm", "tide", "vale",
        "wave", "wisp", "wren", "zephyr"
    ]

    static let musical = [
        "allegro", "aria", "ballad", "cadence", "canon", "chord",
        "coda", "duet", "forte", "fugue", "harmony", "hymn",
        "lilt", "lyric", "melody", "motif", "opus", "prelude",
        "refrain", "rondo", "sonata", "tempo", "trill", "tune",
        "verse", "waltz"
    ]

    static func generate() -> String {
        let m = magical.randomElement()!
        let n = musical.randomElement()!
        return "\(m)-\(n)"
    }
}
