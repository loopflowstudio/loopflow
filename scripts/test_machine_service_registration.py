"""Prove the bundled SMAppService registrar without opening Desktop or touching Work."""

import os
import plistlib
import subprocess
import tempfile
import time
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

PROBE = r"""
#import <Foundation/Foundation.h>
#import <ServiceManagement/ServiceManagement.h>
#include <stdio.h>
#include <unistd.h>
#include <string.h>
int lf_machine_service_register(const char *, bool, char *, size_t);
int main(int argc, const char **argv) {
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
        char error[1024] = {0};
        int code = lf_machine_service_register(
            argv[2], !strcmp(argv[1], "replace"), error, sizeof(error));
        if (code) fprintf(stderr, "%s\n", error);
        return code;
    }
}
"""


def _run(*args: str) -> None:
    subprocess.run(args, check=True, stdin=subprocess.DEVNULL, capture_output=True, text=True)


def _pid(path: Path, prior: str | None = None) -> str:
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        value = path.read_text() if path.exists() else ""
        if value and value != prior:
            return value
        time.sleep(0.05)
    raise RuntimeError("Registered bundled helper did not become ready")


def main() -> None:
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
        (agents / plist_name).write_bytes(
            plistlib.dumps(
                {
                    "Label": label,
                    "BundleProgram": "Contents/MacOS/probe",
                    "ProgramArguments": ["probe", "serve", str(root / "pid")],
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
        try:
            _run(str(executable), "register", plist_name)
            first = _pid(root / "pid")
            _run(str(executable), "register", plist_name)
            assert _pid(root / "pid") == first, "Repeated registration replaced a live helper"
            _run(str(executable), "replace", plist_name)
            assert _pid(root / "pid", first) != first
            print(
                "Signed bundle registered, repeated startup preserved it, "
                "replacement started a new helper."
            )
        finally:
            subprocess.run(
                [str(executable), "remove", plist_name],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )
            subprocess.run(
                ["launchctl", "bootout", f"gui/{os.getuid()}/{label}"],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        raise SystemExit(error.stderr.strip()) from error
