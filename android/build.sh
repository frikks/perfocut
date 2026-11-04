#!/bin/bash
# Собирает dist/PerfoCut.apk без Gradle: Rust-библиотека (Slint) + маленькая Java-обёртка.
# Нужны: Rust с целью aarch64-linux-android, Android NDK, JDK 17,
# Android SDK (platforms;android-34, build-tools;34.0.0).
# Подпись: ANDROID_KEYSTORE_B64 (секрет) или android/test.keystore.b64 из репозитория.
set -euo pipefail
cd "$(dirname "$0")"
ROOT=..
SDK=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-/data/tools/android-sdk}}
BT=$SDK/build-tools/${BUILD_TOOLS:-34.0.0}
AJ=$SDK/platforms/android-34/android.jar
NDK=${ANDROID_NDK_ROOT:-${ANDROID_NDK_HOME:-$(ls -d "$SDK"/ndk/* | sort -V | tail -1)}}
MIN_API=24
VERSION=$(sed -n 's/^version *= *"\([^"]*\)".*/\1/p' $ROOT/Cargo.toml | head -1)
IFS=. read -r MA MI PA <<<"${VERSION%%-*}"
CODE=$(( ${MA:-0}*10000 + ${MI:-0}*100 + ${PA:-0} ))
KS_PASS=${ANDROID_KEYSTORE_PASS:-android}
KS_ALIAS=${ANDROID_KEY_ALIAS:-perfocut}

# 1. Rust → lib/arm64-v8a/libperfocut.so
TC=$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin
export ANDROID_NDK_ROOT=$NDK ANDROID_NDK_HOME=$NDK
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$TC/aarch64-linux-android$MIN_API-clang
export CC_aarch64_linux_android=$TC/aarch64-linux-android$MIN_API-clang
export CXX_aarch64_linux_android=$TC/aarch64-linux-android$MIN_API-clang++
export AR_aarch64_linux_android=$TC/llvm-ar
(cd $ROOT && cargo rustc -p perfocut --lib --release --target aarch64-linux-android --crate-type cdylib)

rm -rf build && mkdir -p build/res build/gen build/classes build/dex build/apk/lib/arm64-v8a
cp $ROOT/target/aarch64-linux-android/release/libperfocut.so build/apk/lib/arm64-v8a/

# 2. Ресурсы, манифест и Java-обёртка
$BT/aapt2 compile --dir res -o build/res/res.zip
$BT/aapt2 link -o build/base.apk -I "$AJ" --manifest AndroidManifest.xml \
  --java build/gen build/res/res.zip --min-sdk-version $MIN_API --target-sdk-version 34 \
  --version-code $CODE --version-name "$VERSION"
javac -nowarn -encoding UTF-8 -source 8 -target 8 -bootclasspath "$AJ" -classpath "$AJ" \
  -d build/classes $(find java build/gen -name '*.java') > build/javac.log 2>&1 || { cat build/javac.log; exit 1; }
$BT/d8 --min-api $MIN_API --release --lib "$AJ" --output build/dex $(find build/classes -name '*.class')

# 3. Упаковка, выравнивание и подпись
cp build/base.apk build/unsigned.apk
(cd build/dex && zip -q -j ../unsigned.apk classes.dex)
(cd build/apk && zip -q -r ../unsigned.apk lib)
$BT/zipalign -f -p 4 build/unsigned.apk build/aligned.apk
if [ -n "${ANDROID_KEYSTORE_B64:-}" ]; then echo "$ANDROID_KEYSTORE_B64" | base64 -d > build/sign.keystore
else base64 -d test.keystore.b64 > build/sign.keystore; fi
mkdir -p $ROOT/dist
$BT/apksigner sign --ks build/sign.keystore --ks-pass pass:$KS_PASS --key-pass pass:$KS_PASS \
  --ks-key-alias $KS_ALIAS --out $ROOT/dist/PerfoCut.apk build/aligned.apk
rm -f $ROOT/dist/PerfoCut.apk.idsig build/sign.keystore
$BT/apksigner verify $ROOT/dist/PerfoCut.apk
echo "PerfoCut.apk $VERSION (code $CODE), $(du -h $ROOT/dist/PerfoCut.apk | cut -f1)"
