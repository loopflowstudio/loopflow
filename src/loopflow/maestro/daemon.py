"""Maestro HTTP server entry point."""

import uvicorn


def main():
    """Run the maestro HTTP server."""
    uvicorn.run(
        "loopflow.maestro.api:app",
        host="127.0.0.1",
        port=8420,
        log_level="info",
    )


if __name__ == "__main__":
    main()
