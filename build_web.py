"""Собирает dist/PerfoCut.html: вставляет ядро (WASM) и версию в web/app.html."""
import base64, pathlib, re
root = pathlib.Path(__file__).resolve().parent
version = re.search(r'^version\s*=\s*"([^"]+)"', (root/'Cargo.toml').read_text(), re.M).group(1)
wasm = (root/'target/wasm32-unknown-unknown/release/perfo_core.wasm').read_bytes()
html = (root/'web/app.html').read_text(encoding='utf-8')
assert '/*WASM_BASE64*/' in html and '/*VERSION*/' in html
out = html.replace('/*WASM_BASE64*/', base64.b64encode(wasm).decode()).replace('/*VERSION*/', version)
(root/'dist').mkdir(exist_ok=True)
(root/'dist/PerfoCut.html').write_text(out, encoding='utf-8')
print('PerfoCut.html', version, len(out))
