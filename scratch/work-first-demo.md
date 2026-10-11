# Work-first native demo — October 11, 2026

Jack Heart invoked demo and selected real configured repositories. This authorizes
this demonstration, not cleanup or landing. Design: [get-me-using-lf.md](get-me-using-lf.md).
Published review: https://github.com/loopflowstudio/loopflow/pull/1531.

Built revision `0baa92dc16` with the repository development launcher and launched
`~/Applications/Loopflow Dev.app` against the Loopflow repository and normal Machine.
The installed production app was left running. The dev CLI forwards normal-Machine
commands to the installed CLI; this does not install branch CLI/schema changes.
Build and launched app share Mach-O UUID `82458694-00C4-3CFB-99D2-F6978A19DBFC`.
Byte comparison differs after app signing, so it is not a binary-parity assertion.

Check: `uv run python scripts/loopflow-dev.py build` and `run` succeeded; the Dev
app process was observed. Build log: `/tmp/loo462-demo-launch.log`. Existing
Ghostty linker/deprecation warnings remain; process launch proves no usable UI.

## Remaining native acceptance

Repository-name opening must reuse the retained conversation without duplication.
Task opening must show the conversation first with a usable compact Overview;
switching Work must retain unsent input and focus. These observations remain absent.
Empty-repository entry is separate: no real empty repository was created or planning
records removed for this demonstration. No data cleanup or landing occurred.

## Failed real-data opening

Jack Heart supplied a screenshot of the Dev app's Open Work alert:
`error: unrecognized subcommand 'identity'`. The repository conversation did not
open. This is a failed demo, not native acceptance of the workspace changes.

The installed `lf 0.13.11` lacks `repo identity` in `lf repo --help`.
`RegistryQuery.repositoryIdentity` calls `repo identity --json`; invoking that
read through the Dev app's bundled CLI reproduces the same error. The bundled
CLI's `repo identity --help` succeeds because it describes branch syntax, while
ordinary main-Machine execution forwards to the installed CLI. Build parity alone
therefore does not establish runtime capability parity. The demo launcher should
have checked the actual installed command path before asking Jack Heart to try it.

Real configured-repository review is blocked on a compatible installed release
containing the LOO-427 foundation, or an explicitly reviewed supported delivery
path for that foundation. Do not add a fake identity fallback, override the Home,
or bypass release/migration authority to make the screenshot disappear. A private
fixture can test the branch together but cannot satisfy Jack Heart's requested
real-repository demo. No installation of the branch CLI or migration was attempted.

## Next pursuit — Jack Heart, October 11

Jack Heart requested stacking on LOO-427 and continued pursuit through publication
for another demo, not landing. Parent `f1e8cd19ff5b` is integrated at `c040d9fa03`;
both focused WorkCache conflict tests passed. Stacking did not supply the compatible
installed runtime. No live migration, Session takeover or data deletion was authorized.

The continuation at `1787d94bc9`, compressed at `df40c477ef`, repairs `run` and `run-debug` to check repository opening through
the bundled helper's ordinary dispatch before launch, retaining the actual error
and naming the compatible-release dependency. The misleading claim that bundled
source always reads branch data is removed. This operation can register a repository;
it is not a read-only capability probe. Headless executable fixtures cover installed
support/refusal and explicit private-Machine launch. No new real-data demo was run.
