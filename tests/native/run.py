"""Build the real native modules with an isolated ignored manifest/target.

Usage: python3 tests/native/run.py [cargo test arguments...]
No device or network API is called by tests; Cargo may fetch build dependencies.
"""
from pathlib import Path
import os
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
LOCAL = ROOT / '.local' / 'native-harness'
LOCAL.mkdir(parents=True, exist_ok=True)
manifest = f'''[package]
name = "focaus-native-validation"
version = "0.1.0"
edition = "2021"
[workspace]
[lib]
path = "{ROOT / 'tests/native/harness.rs'}"
[dependencies]
focaus-contracts = {{ path = "{ROOT / 'crates/focaus-contracts'}" }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
sha2 = "0.10"
rusqlite = {{ version = "=0.32.1", features = ["bundled"] }}
cpal = "=0.16.0"
unicode-normalization = "=0.1.22"
'''
(LOCAL / 'Cargo.toml').write_text(manifest)
env = os.environ.copy()
env['CARGO_HOME'] = str(ROOT / '.local' / 'cargo')
env['CARGO_TARGET_DIR'] = str(ROOT / 'target' / 'native')
cargo = shutil.which('cargo')
if not cargo:
    raise SystemExit('Put the approved Rust toolchain bin on PATH before running.')
raise SystemExit(subprocess.call([cargo, 'test', '--manifest-path', str(LOCAL / 'Cargo.toml'), *sys.argv[1:]], cwd=ROOT, env=env))
