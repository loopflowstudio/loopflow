import Foundation
import Testing
@testable import Loopflow

struct RawJSONTests {
    @Test func fieldsKeepEachValueVerbatim() throws {
        let line = Data(#" { "type" : "reading", "frame": {"body": [1, {"a": "}]\"{"}], "answers": null} , "n": -1.5e3, "ok": true } "#.utf8)
        let fields = try RawJSON(line).fields()
        #expect(fields["type"]?.string == "reading")
        #expect(String(decoding: fields["frame"]!.bytes, as: UTF8.self) == #"{"body": [1, {"a": "}]\"{"}], "answers": null}"#)
        #expect(try fields["frame"]!.fields()["answers"]?.isNull == true)
        #expect(try fields["n"]?.decode(Double.self) == -1500)
        #expect(try fields["ok"]?.decode(Bool.self) == true)
    }

    @Test func escapedStringsDecode() throws {
        let fields = try RawJSON(Data(#"{"message":"a \"quoted\" é line\n"}"#.utf8)).fields()
        #expect(fields["message"]?.string == "a \"quoted\" é line\n")
    }

    @Test func malformedEnvelopesThrow() {
        for text in [#"{"a": "#, #"["a"]"#, #"{"a" 1}"#, #"{"a": {"b": 1}"#, ""] {
            #expect(throws: (any Error).self, "\(text)") { try RawJSON(Data(text.utf8)).fields() }
        }
    }
}
