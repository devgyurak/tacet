#!/usr/bin/env python3
"""만든 .msix가 Store에 올릴 모양인지 확인한다(docs/platform/packaging.md).

    python scripts/check_msix.py target/release/bundle/tacet-<버전>-win-x64.msix [--store]

- 매니페스트(AppxManifest.xml)에 권한 세 가지(runFullTrust, location, bluetooth)가 있는지 — 빠지면 묶음 앱이 위치 권한을
  못 받아 Wi-Fi BSSID가 비거나 BLE를 못 받는다.
- 타일 이미지가 electron-builder 기본(Electron 견본) 그림이 아니라 우리 것인지.
- `--store`: Partner Center 제품 ID로 묶였는지(시험용 기본값 CN=ms 가 아닌지). 릴리스에서 쓴다.

표준 라이브러리만 쓴다(CI의 Windows 실행기 Python).
"""

from __future__ import annotations

import sys
import xml.etree.ElementTree as ET
import zipfile

NS = {
    "m": "http://schemas.microsoft.com/appx/manifest/foundation/windows10",
    "rescap": "http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities",
}
REQUIRED_CAPABILITIES = {"runFullTrust", "location", "bluetooth"}
# 우리 타일 이미지(desktop/build/appx/). electron-builder 견본 그림은 이름이 SampleAppx.* 다.
REQUIRED_ASSETS = {"StoreLogo.png", "Square44x44Logo.png", "Square150x150Logo.png", "Wide310x150Logo.png"}
TEST_PUBLISHER = "CN=ms"
# appx.identityName이 없으면 electron-builder가 package.json의 name을 쓴다.
TEST_IDENTITY_NAME = "tacet-desktop"


def problems(path: str, store: bool) -> list[str]:
    found: list[str] = []
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        manifest = ET.fromstring(archive.read("AppxManifest.xml"))
    capabilities = {
        element.get("Name")
        for element in manifest.iter()
        if element.tag.endswith("Capability") and element.get("Name")
    }
    for missing in sorted(REQUIRED_CAPABILITIES - capabilities):
        found.append(f"매니페스트에 권한이 없다: {missing}")
    assets = {name.replace("\\", "/").rsplit("/", 1)[-1] for name in names if name.replace("\\", "/").startswith("assets/")}
    for missing in sorted(REQUIRED_ASSETS - assets):
        found.append(f"타일 이미지가 없다: assets/{missing}")
    if any(name.startswith("SampleAppx") for name in assets):
        found.append("electron-builder 견본 타일 이미지(SampleAppx.*)가 들어 있다")
    identity = manifest.find("m:Identity", NS)
    publisher = identity.get("Publisher") if identity is not None else None
    print(f"Identity: Name={identity.get('Name') if identity is not None else None} Publisher={publisher}")
    name = identity.get("Name") if identity is not None else None
    if store and publisher == TEST_PUBLISHER:
        found.append("Partner Center 제품 ID 없이 묶였다(Publisher=CN=ms) — TACET_APPX_* 값을 넣는다")
    if store and name == TEST_IDENTITY_NAME:
        found.append(f"Partner Center 제품 ID 없이 묶였다(Name={TEST_IDENTITY_NAME}) — TACET_APPX_IDENTITY_NAME을 넣는다")
    return found


def main(argv: list[str]) -> int:
    # Windows 실행기의 콘솔 인코딩(cp1252)은 한국어를 못 찍는다 — scripts/verify.sh의 PYTHONUTF8과 같은 이유.
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(encoding="utf-8")
    paths = [arg for arg in argv if not arg.startswith("--")]
    if len(paths) != 1:
        print(__doc__, file=sys.stderr)
        return 2
    found = problems(paths[0], store="--store" in argv)
    for line in found:
        print(f"MSIX 확인 실패: {line}", file=sys.stderr)
    if not found:
        print("MSIX 확인: 통과")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
