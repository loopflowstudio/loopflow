"""Exercise signed service registration, optionally hosting a disposable Work proof."""

import argparse
import os
import plistlib
import shutil
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LSREGISTER = (
    "/System/Library/Frameworks/CoreServices.framework/"
    "Frameworks/LaunchServices.framework/Support/lsregister"
)

PROBE = r"""
#import <Foundation/Foundation.h>
#import <ServiceManagement/ServiceManagement.h>
#include <stdio.h>
#include <unistd.h>
#include <string.h>
int lf_machine_service_register(const char *, bool, char *, size_t);
int perform(int argc, const char **argv) {
    @autoreleasepool {
        if (argc < 3) return 2;
        if (!strcmp(argv[1], "serve")) {
            FILE *file = fopen(argv[2], "w");
            fprintf(file, "%d", getpid()); fclose(file);
            while (1) pause();
        }
        if (!strcmp(argv[1], "remove")) {
            NSError *error = nil;
            SMAppService *service = [SMAppService agentServiceWithPlistName:
                [NSString stringWithUTF8String:argv[2]]];
            BOOL ok = [service unregisterAndReturnError:&error];
            if (!ok) fprintf(stderr, "%s\n", error.localizedDescription.UTF8String);
            return ok ? 0 : 1;
        }
        if (!strcmp(argv[1], "replace") && argc >= 4 && argv[3][0]) {
            NSString *helper = [NSBundle.mainBundle.bundlePath
                stringByAppendingPathComponent:@"Contents/MacOS/lf"];
            NSString *pending = [helper stringByAppendingString:@".replacement"];
            NSError *copyError = nil;
            NSString *source = [NSString stringWithUTF8String:argv[3]];
            if (![NSFileManager.defaultManager copyItemAtPath:source
                    toPath:pending error:&copyError]) {
                fprintf(stderr, "Cannot stage helper: %s\n",
                    copyError.localizedDescription.UTF8String);
                return 1;
            }
            if (rename(pending.fileSystemRepresentation, helper.fileSystemRepresentation)) {
                perror("Cannot replace helper"); return 1;
            }
        }
        char error[1024] = {0};
        int code = lf_machine_service_register(
            argv[2], !strcmp(argv[1], "replace"), error, sizeof(error));
        if (code) fprintf(stderr, "%s\n", error);
        return code;
    }
}
int main(int argc, const char **argv) {
    int code = perform(argc, argv);
    if (argc == 5) {
        FILE *file = fopen(argv[4], "w");
        if (!file) { perror("Cannot record registrar result"); return 2; }
        fprintf(file, "%d", code); fclose(file);
    }
    return code;
}
"""


def _run(*args: str) -> None:
    subprocess.run(args, check=True, stdin=subprocess.DEVNULL, capture_output=True, text=True)


def _probe(executable: Path, action: str, plist: str, candidate: Path | None = None) -> None:
    bundle = executable.parent.parent.parent
    result = bundle.parent / "probe-result"
    log = bundle.parent / "probe.log"
    result.unlink(missing_ok=True)
    log.unlink(missing_ok=True)
    # LaunchServices gives the signed app its own execution identity. A child of
    # the test runner cannot act as the app's updater under App Management.
    _run(
        "open",
        "-n",
        "--stdout",
        str(log),
        "--stderr",
        str(log),
        str(bundle),
        "--args",
        action,
        plist,
        str(candidate) if candidate else "",
        str(result),
    )
    # A short-lived updater may exit before `open -W` installs its process
    # watch. Its own result records completion without that launch/wait race.
    deadline = time.monotonic() + 30
    outcome = ""
    while time.monotonic() < deadline:
        outcome = result.read_text() if result.exists() else ""
        if outcome:
            break
        time.sleep(0.05)
    if outcome != "0":
        diagnostic = log.read_text().strip() if log.exists() else ""
        raise RuntimeError(diagnostic or f"Registrar did not succeed: {outcome or 'no result'}")


def _pid(path: Path, prior: str | None = None) -> str:
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        value = path.read_text() if path.exists() else ""
        if value and value != prior:
            return value
        time.sleep(0.05)
    raise RuntimeError("Registered bundled helper did not become ready")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--lf", type=Path)
    parser.add_argument("--home", type=Path)
    args = parser.parse_args()
    if bool(args.lf) != bool(args.home):
        parser.error("--lf and --home must be supplied together")
    label = "com.loopflow.machine-proof." + uuid.uuid4().hex
    with tempfile.TemporaryDirectory(prefix="lf-service-package-") as temporary:
        root = Path(temporary)
        contents = root / "MachineProof.app/Contents"
        executable = contents / "MacOS/probe"
        agents = contents / "Library/LaunchAgents"
        executable.parent.mkdir(parents=True)
        agents.mkdir(parents=True)
        plist_name = label + ".plist"
        (contents / "Info.plist").write_bytes(
            plistlib.dumps(
                {
                    "CFBundleIdentifier": label,
                    "CFBundleExecutable": "probe",
                    "CFBundlePackageType": "APPL",
                    "CFBundleVersion": "1",
                    "LSUIElement": True,
                }
            )
        )
        helper = contents / "MacOS/lf"
        if args.lf:
            shutil.copy2(args.lf, helper)
            _run(
                "codesign", "--force", "--sign", "-", "--identifier", label + ".helper", str(helper)
            )
        (agents / plist_name).write_bytes(
            plistlib.dumps(
                {
                    "Label": label,
                    "BundleProgram": "Contents/MacOS/lf" if args.lf else "Contents/MacOS/probe",
                    "ProgramArguments": [str(helper), "machine", "service", "run"]
                    if args.lf
                    else ["probe", "serve", str(root / "pid")],
                    "EnvironmentVariables": {"LF_HOME": str(args.home), "LF_BIN": str(helper)}
                    if args.lf
                    else {},
                    "StandardOutPath": str(args.home / "installed-helper.log")
                    if args.home
                    else str(root / "helper.log"),
                    "StandardErrorPath": str(args.home / "installed-helper.log")
                    if args.home
                    else str(root / "helper.log"),
                    "RunAtLoad": True,
                    "KeepAlive": True,
                    "AbandonProcessGroup": True,
                }
            )
        )
        source = root / "probe.m"
        source.write_text(PROBE)
        _run(
            "clang",
            "-fobjc-arc",
            "-framework",
            "Foundation",
            "-framework",
            "ServiceManagement",
            str(source),
            str(ROOT / "rust/loopflow/src/machine_service/registration.m"),
            "-o",
            str(executable),
        )
        _run("codesign", "--force", "--sign", "-", str(contents.parent))
        replacement = root / "next-lf"
        if args.lf:
            # Stage signed bytes before registration. The app owns its own
            # replacement under macOS App Management; Python never mutates a
            # registered bundle. Identical signed bytes retain its resource seal.
            shutil.copy2(helper, replacement)
        # SMAppService resolves BundleProgram through the app's registered
        # container. Temporary proof bundles need the installation's LS entry
        # before registering their Background Item, not an indexing race.
        _run(LSREGISTER, "-f", str(contents.parent))
        replaced = False
        try:
            _probe(executable, "register", plist_name)
            if args.lf:
                print(str(helper), flush=True)
                for line in sys.stdin:
                    if line.strip() != "replace":
                        raise RuntimeError("Unknown installation proof command")
                    original_inode = helper.stat().st_ino
                    _probe(executable, "replace", plist_name, replacement)
                    assert helper.stat().st_ino != original_inode
                    _run("codesign", "--verify", "--deep", "--strict", str(contents.parent))
                    replaced = True
                    print("replaced", flush=True)
                return
            first = _pid(root / "pid")
            _probe(executable, "register", plist_name)
            assert _pid(root / "pid") == first, "Repeated registration replaced a live helper"
            _probe(executable, "replace", plist_name)
            assert _pid(root / "pid", first) != first
            print(
                "Signed bundle registered, repeated startup preserved it, "
                "replacement started a new helper."
            )
        finally:
            if args.lf and not replaced:
                diagnostic = subprocess.run(
                    ["launchctl", "print", f"gui/{os.getuid()}/{label}"],
                    capture_output=True,
                    text=True,
                    check=False,
                )
                for line in diagnostic.stdout.splitlines():
                    if line.strip().startswith(
                        ("state =", "pid =", "last exit code =", "last exit reason =", "program =")
                    ):
                        print("launchd: " + line.strip(), file=sys.stderr)
            try:
                _probe(executable, "remove", plist_name)
            except (RuntimeError, subprocess.CalledProcessError):
                pass  # bootout below also removes a job whose registrar exited.
            subprocess.run(
                ["launchctl", "bootout", f"gui/{os.getuid()}/{label}"],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )
            subprocess.run(
                [LSREGISTER, "-u", str(contents.parent)],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        raise SystemExit(
            f"{error.cmd[0]} exited {error.returncode}: {error.stderr.strip()}"
        ) from error
