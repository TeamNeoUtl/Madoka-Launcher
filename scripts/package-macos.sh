#!/usr/bin/env bash
# Builds `Madoka Launcher.app` from a release build and zips it for distribution.
# Usage: bash scripts/package-macos.sh <profile dir, e.g. target/release> <version> <output zip>
set -euo pipefail

profile_dir=${1:?profile directory}
version=${2:?version}
archive=${3:?output zip}
app="Madoka Launcher.app"

# The newest `icon.icns` that the build script of launcher-app wrote.
icon=$(find "$profile_dir/build" -path '*launcher-app-*' -name icon.icns -print0 | xargs -0 ls -t | head -n 1)
test -n "$icon"

rm -rf "$app" "$archive"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$profile_dir/Madoka-Launcher" "$profile_dir/Madoka-Browser" "$app/Contents/MacOS/"
cp "$icon" "$app/Contents/Resources/icon.icns"
cp LICENSE README.md "$app/Contents/Resources/"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>Madoka-Launcher</string>
  <key>CFBundleIdentifier</key><string>io.github.teamneoutl.madoka-launcher</string>
  <key>CFBundleName</key><string>Madoka Launcher</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleIconFile</key><string>icon</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>CFBundleURLTypes</key>
  <array>
    <dict>
      <key>CFBundleURLName</key><string>Madoka Launcher</string>
      <key>CFBundleURLSchemes</key>
      <array><string>madoka</string><string>aviutl2-catalog</string></array>
    </dict>
  </array>
</dict>
</plist>
PLIST

# Apple Silicon kills unsigned code. The ad-hoc identity needs no certificate.
codesign --force --deep -s - "$app"
ditto -c -k --keepParent "$app" "$archive"
