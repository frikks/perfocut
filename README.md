# PerfoCut

Генератор перфорации для лазерной резки листового металла. Из картинки строит узор отверстий или линий, считает стоимость резки и выгружает DXF, G-code (.nc, .cnc), SVG, PDF, EPS, PLT, PNG, CSV и файл заказа `.perfo`.

С версии 0.3 вся программа написана на Rust, интерфейс на [Slint](https://slint.dev). Нет WebView, Edge, Tauri и JavaScript-интерфейса: окно рисует сама программа.

| Файл | Что это |
| --- | --- |
| `PerfoCut.exe` | Windows 10/11 x64, без установки. Обычное окно, рисуется через OpenGL, без него программно. |
| `PerfoCut.apk` | Android 7+, arm64. Нативное приложение, отрисовка через Skia. |
| `PerfoCut.html` | Та же программа в браузере. Внутри WebAssembly: браузер по-другому код на Rust не запускает. Работает офлайн. |

Интерфейс на русском и английском, темы: основная (графит), светлая, как в системе, AMOLED. Настройки открывает шестерёнка в правом верхнем углу.

Скачать: [Releases](../../releases).

## Как выпустить новую версию

1. Поменяйте `version` в корневом `Cargo.toml`, например `0.3.0` → `0.3.1`.
2. Отправьте изменения в `main`.
3. GitHub Actions соберёт три файла и опубликует релиз `v0.3.1`.

Если версия не менялась, сборка только проверяет код. Файлы такой сборки лежат в Actions → запуск → Artifacts.

## Структура

```
core/          ядро: генерация узора, проверки под лазер, расчёт, экспорт, .perfo
app/ui/        интерфейс на Slint: app.slint (окно), panels.slint, widgets.slint, theme.slint
app/src/       логика интерфейса: app.rs (состояние, фоновый расчёт), render.rs (превью),
               files.rs (картинки и заказы), i18n.rs (числа, перевод строк из кода)
app/src/platform/   desktop.rs (Windows и Linux), android.rs, web.rs
app/lang/      перевод на английский (gettext .po, ключи на русском)
android/       MainActivity.java (файлы, вибрация, ссылки) и build.sh: APK без Gradle
web/           build.py и index.html: один HTML-файл со встроенным WebAssembly
```

## Сборка вручную

```bash
cargo test --workspace
cargo run -p perfocut                           # окно на своём компьютере
cargo build --release -p perfocut --target x86_64-pc-windows-gnu   # нужен mingw-w64
bash android/build.sh                           # нужны NDK, JDK 17, Android SDK 34
python3 web/build.py                            # нужен wasm-bindgen той же версии, что в Cargo.lock
```

Готовые файлы появляются в `dist/`.

## Подпись APK

APK подписан тестовым ключом из `android/test.keystore.b64` (пароль `android`). Для своей подписи создайте ключ и добавьте в Settings → Secrets → Actions:

- `ANDROID_KEYSTORE_B64`: файл ключа в base64;
- `ANDROID_KEYSTORE_PASS`: пароль;
- `ANDROID_KEY_ALIAS`: имя ключа.

После смены ключа старую версию на телефоне придётся один раз удалить.
