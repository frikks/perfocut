"""Собирает dist/PerfoCut.html: вставляет в web/app.html скрипты, ядро (WASM) и версию."""
import base64, pathlib, re
root = pathlib.Path(__file__).resolve().parent
version = re.search(r'^version\s*=\s*"([^"]+)"', (root/'Cargo.toml').read_text(), re.M).group(1)
wasm = (root/'target/wasm32-unknown-unknown/release/perfo_core.wasm').read_bytes()
web = root/'web'
html = (web/'app.html').read_text(encoding='utf-8')
for mark in ('/*WASM_BASE64*/', '/*VERSION*/', '/*I18N_JS*/', '/*APP_JS*/'):
    assert mark in html, mark
# скрипты вставляем первыми, затем версию (она встречается и в скриптах)
html = html.replace('/*I18N_JS*/', (web/'i18n.js').read_text(encoding='utf-8'))
html = html.replace('/*APP_JS*/', (web/'app.js').read_text(encoding='utf-8'))
html = html.replace('/*VERSION*/', version)
out = html.replace('/*WASM_BASE64*/', base64.b64encode(wasm).decode())
(root/'dist').mkdir(exist_ok=True)
(root/'dist/PerfoCut.html').write_text(out, encoding='utf-8')
print('PerfoCut.html', version, len(out))
