import Testing
@testable import Concerto

@Suite("DiffLinesView")
struct DiffLinesViewTests {
    @Test("Empty input returns no lines")
    func emptyInput() {
        let lines = parseDiffLines("")
        #expect(lines.isEmpty)
    }

    @Test("Addition lines start with + but not +++")
    func additionLines() {
        let diff = "+added line"
        let lines = parseDiffLines(diff)
        #expect(lines.count == 1)
        #expect(lines[0].kind == .addition)
        #expect(lines[0].text == "+added line")
    }

    @Test("Deletion lines start with - but not ---")
    func deletionLines() {
        let diff = "-removed line"
        let lines = parseDiffLines(diff)
        #expect(lines.count == 1)
        #expect(lines[0].kind == .deletion)
        #expect(lines[0].text == "-removed line")
    }

    @Test("Hunk headers start with @@")
    func hunkHeaders() {
        let diff = "@@ -1,5 +1,7 @@"
        let lines = parseDiffLines(diff)
        #expect(lines.count == 1)
        #expect(lines[0].kind == .hunk)
    }

    @Test("File headers start with --- or +++")
    func fileHeaders() {
        let diff = """
        --- a/src/foo.swift
        +++ b/src/foo.swift
        """
        let lines = parseDiffLines(diff)
        #expect(lines.count == 2)
        #expect(lines[0].kind == .header)
        #expect(lines[1].kind == .header)
    }

    @Test("Context lines have no prefix")
    func contextLines() {
        let diff = " unchanged line"
        let lines = parseDiffLines(diff)
        #expect(lines.count == 1)
        #expect(lines[0].kind == .context)
    }

    @Test("Mixed diff parses all line kinds")
    func mixedDiff() {
        let diff = """
        --- a/file.swift
        +++ b/file.swift
        @@ -1,3 +1,4 @@
         context
        -old line
        +new line
        +extra line
        """
        let lines = parseDiffLines(diff)
        #expect(lines.count == 7)
        #expect(lines[0].kind == .header)
        #expect(lines[1].kind == .header)
        #expect(lines[2].kind == .hunk)
        #expect(lines[3].kind == .context)
        #expect(lines[4].kind == .deletion)
        #expect(lines[5].kind == .addition)
        #expect(lines[6].kind == .addition)
    }

    @Test("Multi-file diff concatenated with newline")
    func multiFileDiff() {
        let diff = """
        --- a/first.swift
        +++ b/first.swift
        @@ -1,2 +1,2 @@
        -old
        +new
        --- a/second.swift
        +++ b/second.swift
        @@ -1,2 +1,2 @@
        -alpha
        +beta
        """
        let lines = parseDiffLines(diff)

        let headers = lines.filter { $0.kind == .header }
        let hunks = lines.filter { $0.kind == .hunk }
        let additions = lines.filter { $0.kind == .addition }
        let deletions = lines.filter { $0.kind == .deletion }

        #expect(headers.count == 4)
        #expect(hunks.count == 2)
        #expect(additions.count == 2)
        #expect(deletions.count == 2)
    }

    @Test("Line IDs are sequential indices")
    func sequentialIds() {
        let diff = "+a\n-b\n c"
        let lines = parseDiffLines(diff)
        #expect(lines.map(\.id) == [0, 1, 2])
    }
}
