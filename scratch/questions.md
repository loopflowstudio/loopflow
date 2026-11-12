# LOO-809

Visual inspection remains for QA; it does not block headless work.

`docs/server.md` phone access now documents `lf server mobile` as shipped,
since the remote carrier and iOS client (`swift/LoopflowiOS`, `/v1/ticket`)
are on main. If phone access should stay unadvertised until an iOS release,
restore the "planned" wording there.

Twenty attempts between November 12 01:17 and 16:04 UTC of
`lf open --host desktop --file docs/server.md --file
rust/loopflow/src/server/README.md` returned:
`No Desktop Files view attached; nothing opened.`
Earlier attempts are recorded through
`139c487f74817090775abe7496ae75eaa4d3a8f6:scratch/questions.md`.
