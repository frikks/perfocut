#!/usr/bin/env python3
"""Собирает dist/PerfoCut.html: та же программа на Rust, собранная в WebAssembly.

Один файл без внешних ссылок: открывается двойным щелчком, работает без интернета.
Нужны: цель wasm32-unknown-unknown и wasm-bindgen той же версии, что в Cargo.lock.
"""
import base64
import gzip
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "target" / "web"
WASM = ROOT / "target" / "wasm32-unknown-unknown" / "release" / "perfocut.wasm"


def run(*cmd: str) -> None:
    print("+", " ".join(cmd), flush=True)
    subprocess.run(cmd, cwd=ROOT, check=True)


def version() -> str:
    m = re.search(r'^version\s*=\s*"([^"]+)"', (ROOT / "Cargo.toml").read_text(), re.M)
    return m.group(1) if m else "0.0.0"


def main() -> int:
    if "--no-cargo" not in sys.argv:
        run("cargo", "rustc", "-p", "perfocut", "--lib", "--release",
            "--target", "wasm32-unknown-unknown", "--crate-type", "cdylib")
    shutil.rmtree(OUT, ignore_errors=True)
    run("wasm-bindgen", "--target", "no-modules", "--no-typescript",
        "--out-dir", str(OUT), str(WASM))
    wasm = OUT / "perfocut_bg.wasm"
    if shutil.which("wasm-opt"):
        run("wasm-opt", "-Oz", "--enable-bulk-memory", "--enable-nontrapping-float-to-int",
            "-o", str(wasm), str(wasm))
    glue = (OUT / "perfocut.js").read_text(encoding="utf-8").replace("</script", "<\\/script")
    packed = base64.b64encode(gzip.compress(wasm.read_bytes(), 9, mtime=0)).decode()
    page = (Path(__file__).parent / "index.html").read_text(encoding="utf-8")
    page = page.replace("{{VERSION}}", version()).replace("{{GLUE}}", glue).replace("{{WASM_GZ_B64}}", packed)
    dist = ROOT / "dist"
    dist.mkdir(exist_ok=True)
    (dist / "PerfoCut.html").write_text(page, encoding="utf-8")
    size = (dist / "PerfoCut.html").stat().st_size / 1e6
    print(f"PerfoCut.html {version()}: {size:.1f} МБ (wasm {wasm.stat().st_size / 1e6:.1f} МБ)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
