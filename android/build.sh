#!/bin/bash
# Собирает dist/PerfoCut.apk из dist/PerfoCut.html без Gradle.
# Нужны: JDK 17, Android SDK (platforms;android-34, build-tools;34.0.0).
# Подпись: ANDROID_KEYSTORE_B64 (секрет) или android/test.keystore.b64 из репозитория.
set -euo pipefail
cd "$(dirname "$0")"
ROOT=..
SDK=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-/data/tools/android-sdk}}
BT=$SDK/build-tools/${BUILD_TOOLS:-34.0.0}
AJ=$SDK/platforms/android-34/android.jar
VERSION=$(sed -n 's/^version *= *"\([^"]*\)".*/\1/p' $ROOT/Cargo.toml | head -1)
IFS=. read -r MA MI PA <<<"${VERSION%%-*}"
CODE=$(( ${MA:-0}*10000 + ${MI:-0}*100 + ${PA:-0} ))
KS_PASS=${ANDROID_KEYSTORE_PASS:-android}
KS_ALIAS=${ANDROID_KEY_ALIAS:-perfocut}

rm -rf build && mkdir -p build/res build/gen build/classes build/dex assets
cp $ROOT/dist/PerfoCut.html assets/index.html
$BT/aapt2 compile --dir res -o build/res/res.zip
$BT/aapt2 link -o build/base.apk -I "$AJ" --manifest AndroidManifest.xml -A assets \
  --java build/gen build/res/res.zip --min-sdk-version 24 --target-sdk-version 34 \
  --version-code $CODE --version-name "$VERSION"
javac -nowarn -encoding UTF-8 -source 8 -target 8 -bootclasspath "$AJ" -classpath "$AJ" \
  -d build/classes $(find src build/gen -name '*.java') > build/javac.log 2>&1 || { cat build/javac.log; exit 1; }
$BT/d8 --min-api 24 --release --lib "$AJ" --output build/dex $(find build/classes -name '*.class')
cp build/base.apk build/unsigned.apk
(cd build/dex && zip -q -j ../unsigned.apk classes.dex)
$BT/zipalign -f -p 4 build/unsigned.apk build/aligned.apk

if [ -n "${ANDROID_KEYSTORE_B64:-}" ]; then echo "$ANDROID_KEYSTORE_B64" | base64 -d > build/sign.keystore
else base64 -d test.keystore.b64 > build/sign.keystore; fi
mkdir -p $ROOT/dist
$BT/apksigner sign --ks build/sign.keystore --ks-pass pass:$KS_PASS --key-pass pass:$KS_PASS \
  --ks-key-alias $KS_ALIAS --out $ROOT/dist/PerfoCut.apk build/aligned.apk
rm -f $ROOT/dist/PerfoCut.apk.idsig build/sign.keystore
$BT/apksigner verify $ROOT/dist/PerfoCut.apk
echo "PerfoCut.apk $VERSION (code $CODE)"
