"""Build an ad-hoc signed local validation app; never installs or publishes it.

Run with Node/pnpm and the project's isolated Cargo toolchain on PATH.
The bundled UI is embedded so this app cannot accidentally consume a worker's
Vite server or load the explicit browser fixture route.
"""
from pathlib import Path
import plistlib
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def build():
    subprocess.run(["pnpm", "--filter", "@focaus/desktop", "build"], cwd=ROOT, check=True)
    subprocess.run(["cargo", "build", "--locked", "-p", "focaus-desktop",
                    "--features", "native-shell,custom-protocol", "--bin", "focaus"],
                   cwd=ROOT, check=True)
    app = ROOT / ".local/macos/focaus.app"
    executable = app / "Contents/MacOS/focaus"
    executable.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / "target/debug/focaus", executable)
    with (ROOT / "apps/desktop/src-tauri/Info.plist").open("rb") as file:
        properties = plistlib.load(file)
    properties.update({
        "CFBundleExecutable": "focaus", "CFBundleIdentifier": "com.focaus.dev",
        "CFBundleName": "focaus", "CFBundleDisplayName": "focaus",
        "CFBundlePackageType": "APPL", "CFBundleShortVersionString": "0.1.0",
        "CFBundleVersion": "1", "NSHighResolutionCapable": True,
        "LSMinimumSystemVersion": "13.0",
    })
    with (app / "Contents/Info.plist").open("wb") as file:
        plistlib.dump(properties, file)
    subprocess.run(["codesign", "--force", "--sign", "-", str(app)], check=True)
    subprocess.run(["codesign", "--verify", "--strict", str(app)], check=True)
    print(app)


if __name__ == "__main__":
    build()
