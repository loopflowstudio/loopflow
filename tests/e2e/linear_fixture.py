"""Disposable TLS trust for real Linear-client subprocess fixtures (Linux)."""

import ssl
import subprocess
from pathlib import Path


def tls_context(root: Path) -> tuple[ssl.SSLContext, Path]:
    # A distinct issuer and explicit signing/key IDs also work with strict TLS verifiers.
    cert, key = root / "cert.pem", root / "tls.key"
    subprocess.run(
        [
            "openssl",
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "1",
            "-keyout",
            str(key),
            "-out",
            str(cert),
            "-subj",
            "/CN=Loopflow fixture CA",
            "-addext",
            "basicConstraints=critical,CA:TRUE",
            "-addext",
            "keyUsage=critical,keyCertSign,cRLSign",
            "-addext",
            "subjectKeyIdentifier=hash",
        ],
        check=True,
        capture_output=True,
    )
    tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    leaf, leaf_key, csr = root / "leaf.pem", root / "leaf.key", root / "leaf.csr"
    subprocess.run(
        [
            "openssl",
            "req",
            "-new",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            str(leaf_key),
            "-out",
            str(csr),
            "-subj",
            "/CN=api.linear.app",
        ],
        check=True,
        capture_output=True,
    )
    extensions = root / "extensions"
    extensions.write_text(
        "basicConstraints=critical,CA:FALSE\nsubjectAltName=DNS:api.linear.app\n"
        "authorityKeyIdentifier=keyid:always\n"
    )
    subprocess.run(
        [
            "openssl",
            "x509",
            "-req",
            "-in",
            str(csr),
            "-CA",
            str(cert),
            "-CAkey",
            str(key),
            "-CAcreateserial",
            "-out",
            str(leaf),
            "-days",
            "1",
            "-extfile",
            str(extensions),
        ],
        check=True,
        capture_output=True,
    )
    tls.load_cert_chain(leaf, leaf_key)
    return tls, cert
