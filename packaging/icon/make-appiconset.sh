#!/usr/bin/env bash
# Writes xcode/Assets.xcassets/AppIcon.appiconset (the Xcode project's App Icon) from
# packaging/icon/icon-1024.png: the ten macOS sizes plus Contents.json. Run it after
# redrawing the icon (crates/render/examples/app_icon.rs) and commit the result.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SRC="$ROOT/packaging/icon/icon-1024.png"
CAT="$ROOT/xcode/Assets.xcassets"
SET="$CAT/AppIcon.appiconset"
mkdir -p "$SET"
printf '{\n  "info" : { "author" : "xcode", "version" : 1 }\n}\n' >"$CAT/Contents.json"
images=""
for s in 16 32 128 256 512; do
  for scale in 1 2; do
    px=$((s * scale))
    suffix=""
    if ((scale == 2)); then suffix="@2x"; fi
    name="icon_${s}x${s}${suffix}.png"
    sips -z "$px" "$px" "$SRC" --out "$SET/$name" >/dev/null
    images+="    { \"filename\" : \"$name\", \"idiom\" : \"mac\", \"scale\" : \"${scale}x\", \"size\" : \"${s}x${s}\" },"$'\n'
  done
done
printf '{\n  "images" : [\n%s  ],\n  "info" : { "author" : "xcode", "version" : 1 }\n}\n' "${images%,$'\n'}"$'\n' >"$SET/Contents.json"
echo "wrote $SET"
