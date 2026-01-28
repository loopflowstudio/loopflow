"""Generate Python bindings from proto files.

Run with: python -m loopflow.proto.generate
"""

import subprocess
import sys
from pathlib import Path


def main():
    # Find repo root by looking for pyproject.toml
    current = Path(__file__).parent
    while current != current.parent:
        if (current / "pyproject.toml").exists():
            repo_root = current
            break
        current = current.parent
    else:
        print("Could not find repo root (pyproject.toml)", file=sys.stderr)
        sys.exit(1)

    proto_dir = repo_root / "proto"
    output_dir = Path(__file__).parent

    if not proto_dir.exists():
        print(f"Proto directory not found: {proto_dir}", file=sys.stderr)
        sys.exit(1)

    # Proto files to compile
    proto_files = [
        "loopflow/control/v1/control.proto",
        "loopflow/engine/v1/engine.proto",
    ]

    for proto_file in proto_files:
        proto_path = proto_dir / proto_file
        if not proto_path.exists():
            print(f"Proto file not found: {proto_path}", file=sys.stderr)
            sys.exit(1)

    # Generate Python bindings
    cmd = [
        sys.executable,
        "-m",
        "grpc_tools.protoc",
        f"-I{proto_dir}",
        f"--python_out={output_dir}",
        f"--grpc_python_out={output_dir}",
        f"--pyi_out={output_dir}",
    ] + [str(proto_dir / f) for f in proto_files]

    print("Generating Python bindings...")
    print(f"  Proto dir: {proto_dir}")
    print(f"  Output dir: {output_dir}")
    print(f"  Command: {' '.join(cmd)}")

    result = subprocess.run(cmd, capture_output=True, text=True)

    if result.returncode != 0:
        print("Error generating bindings:", file=sys.stderr)
        print(result.stderr, file=sys.stderr)
        sys.exit(1)

    # Fix imports in generated files (grpc_tools generates absolute imports)
    # The generated files use 'from loopflow.control.v1 import control_pb2'
    # but we need 'from loopflow.proto.loopflow.control.v1 import control_pb2'
    _fix_imports(output_dir)

    print("Done!")


def _fix_imports(output_dir: Path):
    """Fix import paths in generated *_grpc.py files."""
    for grpc_file in output_dir.rglob("*_pb2_grpc.py"):
        content = grpc_file.read_text()

        # Fix imports like 'from loopflow.control.v1 import control_pb2'
        # to 'from loopflow.proto.loopflow.control.v1 import control_pb2'
        fixed = content.replace("from loopflow.", "from loopflow.proto.loopflow.")

        if fixed != content:
            grpc_file.write_text(fixed)
            print(f"  Fixed imports in: {grpc_file.name}")


if __name__ == "__main__":
    main()
