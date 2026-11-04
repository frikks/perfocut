# PerfoCut

Генератор перфорации для лазерной резки листового металла. Из одной картинки строит узор отверстий или линий и выгружает его в DXF, G-code (.nc, .cnc), SVG, PDF, EPS, PLT, PNG, CSV и файл заказа `.perfo` с расчётом стоимости резки.

Ядро написано на Rust и одинаково работает во всех трёх сборках:

| Сборка | Что это |
| --- | --- |
| `PerfoCut.html` | Одна страница, работает офлайн в Chrome или Edge. Ядро внутри в виде WebAssembly. |
| `PerfoCut.exe` | Windows x64, без установки. Открывает тот же интерфейс в окне Edge. |
| `PerfoCut.apk` | Android 7+. Тот же интерфейс внутри приложения. |

Скачать: раздел [Releases](../../releases).

## Как выпустить новую версию

1. Поменяйте `version` в корневом `Cargo.toml`, например `0.1.0` → `0.2.0`.
2. Отправьте изменения в ветку `main`.
3. GitHub Actions соберёт три файла и опубликует релиз `v0.2.0`.

Если версия не менялась, сборка только проверяет код. Файлы такой сборки лежат в Actions → запуск → Artifacts.

## Структура

```
core/       ядро на Rust: генерация, проверки под лазер, расчёт, экспорт
desktop/    Windows .exe: локальный сервер + окно Edge
web/        интерфейс (app.html), build_web.py вставляет в него ядро
android/    приложение на WebView, сборка без Gradle (build.sh)
.github/    автоматический релиз
```

## Сборка вручную

```bash
rustup target add wasm32-unknown-unknown x86_64-pc-windows-gnu
cargo test -p perfo_core --release
cargo build --release -p perfo_core --target wasm32-unknown-unknown
python3 build_web.py                                   # dist/PerfoCut.html
cargo build --release -p perfocut --target x86_64-pc-windows-gnu   # нужен mingw-w64
android/build.sh                                       # нужны JDK 17 и Android SDK 34
```

## Подпись APK

Сейчас APK подписан тестовым ключом из `android/test.keystore.b64` (пароль `android`). Для настоящей подписи создайте свой ключ и добавьте в Settings → Secrets → Actions:

- `ANDROID_KEYSTORE_B64`: файл ключа в base64;
- `ANDROID_KEYSTORE_PASS`: пароль;
- `ANDROID_KEY_ALIAS`: имя ключа.

После смены ключа старую версию на телефоне придётся удалить один раз.
