"""
Figma API client for exporting design assets.

Usage:
    python figma.py list <file_url>           # List all exportable nodes
    python figma.py export <file_url> [node]  # Export node(s) as PNG/SVG

Requires FIGMA_TOKEN environment variable (get from Figma > Settings > Personal access tokens)
"""

import os
import re
import sys
import urllib.request
import urllib.parse
import json
from pathlib import Path


FIGMA_API = "https://api.figma.com/v1"


def get_token():
    token = os.environ.get("FIGMA_TOKEN")
    if not token:
        print("Error: FIGMA_TOKEN environment variable not set")
        print("Get your token from: Figma > Settings > Personal access tokens")
        sys.exit(1)
    return token


def parse_figma_url(url: str) -> tuple[str, str | None]:
    """Extract file key and node ID from a Figma URL."""
    # https://www.figma.com/design/ZusAUvXyxOGdyuUra2TtUb/Loopflow-logo?node-id=0-1
    # https://www.figma.com/file/ZusAUvXyxOGdyuUra2TtUb/Name

    match = re.search(r'figma\.com/(?:design|file)/([a-zA-Z0-9]+)', url)
    if not match:
        print(f"Error: Could not parse Figma URL: {url}")
        sys.exit(1)

    file_key = match.group(1)

    # Check for node-id parameter
    node_match = re.search(r'node-id=([0-9]+-[0-9]+)', url)
    node_id = node_match.group(1) if node_match else None

    return file_key, node_id


def api_request(endpoint: str, token: str) -> dict:
    """Make a request to the Figma API."""
    url = f"{FIGMA_API}{endpoint}"
    req = urllib.request.Request(url, headers={"X-Figma-Token": token})

    try:
        with urllib.request.urlopen(req) as response:
            return json.loads(response.read().decode())
    except urllib.error.HTTPError as e:
        print(f"API Error: {e.code} - {e.reason}")
        if e.code == 403:
            print("Check that your FIGMA_TOKEN is valid and has access to this file")
        sys.exit(1)


def get_file(file_key: str, token: str) -> dict:
    """Get file metadata and structure."""
    return api_request(f"/files/{file_key}", token)


def get_file_nodes(file_key: str, node_ids: list[str], token: str) -> dict:
    """Get specific nodes from a file."""
    ids = ",".join(node_ids)
    return api_request(f"/files/{file_key}/nodes?ids={ids}", token)


def export_nodes(file_key: str, node_ids: list[str], token: str,
                 format: str = "png", scale: float = 2.0) -> dict:
    """Export nodes as images. Returns URLs to download."""
    ids = ",".join(node_ids)
    return api_request(
        f"/images/{file_key}?ids={ids}&format={format}&scale={scale}",
        token
    )


def download_image(url: str, output_path: Path):
    """Download an image from URL to local path."""
    urllib.request.urlretrieve(url, output_path)
    print(f"  Saved: {output_path}")


def walk_nodes(node: dict, depth: int = 0) -> list[tuple[str, str, str, int]]:
    """Walk the node tree and return (id, name, type, depth) tuples."""
    results = []

    node_id = node.get("id", "")
    name = node.get("name", "")
    node_type = node.get("type", "")

    # Only include exportable types
    exportable = {"FRAME", "COMPONENT", "INSTANCE", "GROUP", "VECTOR",
                  "RECTANGLE", "ELLIPSE", "TEXT", "CANVAS"}

    if node_type in exportable:
        results.append((node_id, name, node_type, depth))

    # Recurse into children
    for child in node.get("children", []):
        results.extend(walk_nodes(child, depth + 1))

    return results


def list_nodes(file_key: str, token: str):
    """List all exportable nodes in a file."""
    print(f"Fetching file structure...")
    data = get_file(file_key, token)

    file_name = data.get("name", "Unknown")
    print(f"\nFile: {file_name}")
    print(f"{'='*60}\n")

    document = data.get("document", {})
    nodes = walk_nodes(document)

    for node_id, name, node_type, depth in nodes:
        indent = "  " * depth
        # Format node_id for URL (replace : with -)
        url_id = node_id.replace(":", "-")
        print(f"{indent}{name} ({node_type}) [node-id={url_id}]")

    print(f"\n{'='*60}")
    print(f"Total exportable nodes: {len(nodes)}")
    print(f"\nTo export: python figma.py export <url> --node <node-id>")


def export_to_file(file_key: str, node_ids: list[str], token: str,
                   output_dir: Path, format: str = "png", scale: float = 2.0):
    """Export nodes and save to files."""
    print(f"Exporting {len(node_ids)} node(s) as {format.upper()} at {scale}x...")

    # Convert node IDs from URL format (0-1) to API format (0:1)
    api_ids = [nid.replace("-", ":") for nid in node_ids]

    result = export_nodes(file_key, api_ids, token, format, scale)

    images = result.get("images", {})
    if not images:
        print("No images returned. Check that the node IDs are valid.")
        return

    output_dir.mkdir(parents=True, exist_ok=True)

    # Get node names for filenames
    file_data = get_file_nodes(file_key, api_ids, token)
    nodes_data = file_data.get("nodes", {})

    for api_id, url in images.items():
        if url is None:
            print(f"  Warning: No image for node {api_id}")
            continue

        # Get node name for filename
        node_info = nodes_data.get(api_id, {}).get("document", {})
        name = node_info.get("name", api_id.replace(":", "-"))
        # Clean filename
        safe_name = re.sub(r'[^\w\-]', '_', name).lower()

        output_path = output_dir / f"{safe_name}.{format}"
        download_image(url, output_path)


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        sys.exit(1)

    command = sys.argv[1]
    url = sys.argv[2]

    token = get_token()
    file_key, url_node_id = parse_figma_url(url)

    if command == "list":
        list_nodes(file_key, token)

    elif command == "export":
        # Parse additional arguments
        node_ids = []
        format = "png"
        scale = 2.0
        output_dir = Path("static")

        i = 3
        while i < len(sys.argv):
            arg = sys.argv[i]
            if arg in ("--node", "-n"):
                node_ids.append(sys.argv[i + 1])
                i += 2
            elif arg in ("--format", "-f"):
                format = sys.argv[i + 1]
                i += 2
            elif arg in ("--scale", "-s"):
                scale = float(sys.argv[i + 1])
                i += 2
            elif arg in ("--output", "-o"):
                output_dir = Path(sys.argv[i + 1])
                i += 2
            else:
                i += 1

        # Use node from URL if no --node specified
        if not node_ids and url_node_id:
            node_ids = [url_node_id]

        if not node_ids:
            print("Error: No node ID specified. Use --node <id> or include node-id in URL")
            print("Run 'python figma.py list <url>' to see available nodes")
            sys.exit(1)

        export_to_file(file_key, node_ids, token, output_dir, format, scale)

    else:
        print(f"Unknown command: {command}")
        print(__doc__)
        sys.exit(1)


if __name__ == "__main__":
    main()
