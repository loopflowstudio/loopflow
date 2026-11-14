from __future__ import annotations

import hashlib
import json
import shutil
import zipfile
from pathlib import Path

import pytest

from scripts import third_party


def _write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value))


def _packaged_materials(root: Path) -> Path:
    notice = root / "ThirdPartyNotices"
    sources = root / "ThirdPartySources"
    sources.mkdir(parents=True)
    notice.mkdir()
    (notice / "NOTICES.txt").write_text("Example license")
    source = b"example corresponding source"
    component = {
        "id": "example",
        "scope": "GhosttyKit",
        "license": "MPL-2.0",
        "source": {"sha256": hashlib.sha256(source).hexdigest()},
    }
    _write_json(notice / "inventory.json", {"components": [component]})
    (sources / "example.tar.gz").write_bytes(source)
    _write_json(
        notice / "contents.json",
        {
            str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in root.rglob("*")
            if p.is_file()
        },
    )
    return root


@pytest.mark.parametrize(
    "missing",
    [
        "ThirdPartyNotices/NOTICES.txt",
        "ThirdPartySources/example.tar.gz",
    ],
)
def test_final_bundle_rejects_missing_required_material(tmp_path: Path, missing: str) -> None:
    bundle = _packaged_materials(tmp_path / "Loopflow.app/Contents/Resources")
    third_party.verify(bundle)
    (bundle / missing).unlink()
    with pytest.raises(ValueError, match="Missing or corrupted"):
        third_party.verify(bundle)


def test_archive_is_verified_after_compression(tmp_path: Path) -> None:
    framework = _packaged_materials(tmp_path / "GhosttyKit.xcframework")
    archive = tmp_path / "GhosttyKit.zip"
    with zipfile.ZipFile(archive, "w") as package:
        for path in framework.rglob("*"):
            if path.is_file():
                package.write(path, str(path.relative_to(tmp_path)))
    third_party.verify_archive(archive)
    with (
        pytest.warns(UserWarning, match="Duplicate name"),
        zipfile.ZipFile(archive, "a") as package,
    ):
        # A changed receipt cannot excuse source that differs from the inventory.
        package.writestr("GhosttyKit.xcframework/ThirdPartySources/example.tar.gz", "wrong source")
    with pytest.raises(ValueError, match="Missing or corrupted"):
        third_party.verify_archive(archive)


def test_a_framework_carries_only_its_own_scope_sources(tmp_path: Path) -> None:
    framework = _packaged_materials(tmp_path / "GhosttyKit.xcframework")
    inventory_path = framework / "ThirdPartyNotices/inventory.json"
    inventory = json.loads(inventory_path.read_text())
    inventory["components"].append(
        {"id": "crate@1.0.0", "scope": "lf", "license": "MPL-2.0", "source": {"sha256": "0" * 64}}
    )
    _write_json(inventory_path, inventory)
    receipt = framework / "ThirdPartyNotices/contents.json"
    data = json.loads(receipt.read_text())
    data["ThirdPartyNotices/inventory.json"] = third_party._sha(inventory_path)
    _write_json(receipt, data)
    third_party.verify(framework, "GhosttyKit")
    with pytest.raises(ValueError, match="Incomplete"):
        third_party.verify(framework)


def test_packaging_receipt_cannot_omit_required_sources(tmp_path: Path) -> None:
    bundle = _packaged_materials(tmp_path)
    receipt = bundle / "ThirdPartyNotices/contents.json"
    data = json.loads(receipt.read_text())
    del data["ThirdPartySources/example.tar.gz"]
    _write_json(receipt, data)
    with pytest.raises(ValueError, match="Incomplete"):
        third_party.verify(bundle)


def test_only_mpl_sources_ship(tmp_path: Path) -> None:
    bundle = _packaged_materials(tmp_path)
    inventory_path = bundle / "ThirdPartyNotices/inventory.json"
    inventory = json.loads(inventory_path.read_text())
    inventory["components"].append(
        {
            "id": "permissive",
            "scope": "GhosttyKit",
            "license": "MIT",
            "source": {"sha256": "0" * 64},
        }
    )
    _write_json(inventory_path, inventory)
    receipt = bundle / "ThirdPartyNotices/contents.json"
    data = json.loads(receipt.read_text())
    data["ThirdPartyNotices/inventory.json"] = third_party._sha(inventory_path)
    _write_json(receipt, data)
    third_party.verify(bundle)


def test_shipped_inventory_ships_source_for_its_mpl_components() -> None:
    sourced = {c["id"].split("@")[0] for c in third_party._sourced(third_party.check(), None)}
    assert sourced == {"z2d", "option-ext"}


def test_shipped_inputs_have_current_notices() -> None:
    inventory = third_party.check()
    names = {c["id"] for c in inventory["components"]}
    assert {"ghostty", "z2d", "nerd-glyphs", "app-fonts"} <= names
    assert any(name.startswith("option-ext@") for name in names)


def test_resource_update_invalidates_inventory(tmp_path: Path) -> None:
    root = third_party.ROOT
    paths = third_party._inputs(root)
    for name in paths:
        target = tmp_path / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root / name, target)
    shutil.copytree(root / third_party.NOTICES, tmp_path / third_party.NOTICES)
    font = tmp_path / "swift/Loopflow/Fonts/JetBrainsMono-Regular.ttf"
    font.write_bytes(font.read_bytes() + b"changed")
    with pytest.raises(ValueError, match="inputs changed"):
        third_party.check(tmp_path)
