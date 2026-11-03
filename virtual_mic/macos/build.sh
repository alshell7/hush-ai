#!/bin/sh
# Build the Hush AI Microphone plug-in bundle for Apple Silicon and Intel Macs.
#   virtual_mic/macos/build.sh [output directory, default target]
# Install (needs an administrator):
#   sudo cp -R target/HushMic.driver /Library/Audio/Plug-Ins/HAL/ && sudo killall coreaudiod
set -eu
here=$(cd "$(dirname "$0")" && pwd)
out=${1:-target}
bundle="$out/HushMic.driver"
rm -rf "$bundle"
mkdir -p "$bundle/Contents/MacOS"
cp "$here/Info.plist" "$bundle/Contents/Info.plist"
clang -bundle -O2 -Wall -Wextra -Werror -arch arm64 -arch x86_64 -mmacosx-version-min=11.0 \
    -framework CoreAudio -framework CoreFoundation \
    -o "$bundle/Contents/MacOS/HushMic" "$here/HushMic.c"
# Ad-hoc signature: required on Apple Silicon. Release builds need a Developer ID signature.
codesign --force --sign - "$bundle"
echo "$bundle"
