#!/bin/sh
# Fetch the two models raven-faced runs, and refuse anything that is not them.
#
# They are not vendored in this repository because one of them is 37 MB, and
# they are not fetched at build time because a build that reaches the network
# for something it will then trust with logins is a build nobody can reproduce.
# This is a separate, explicit step, and it checks a hash.
#
# Both come from the OpenCV Zoo, are Apache-2.0 licensed, and are stored there
# with git-lfs -- which is why the URLs are media.githubusercontent.com and not
# raw.githubusercontent.com. The raw host serves a 130-byte pointer file, and a
# pointer file is not a model; the hash check below catches that too.
#
#   sh fetch-models.sh [destination]
#
set -eu

DEST="${1:-/usr/share/raven-face/models}"
BASE="https://media.githubusercontent.com/media/opencv/opencv_zoo/main/models"

# The detector: finds faces and gives five landmarks. 230 KB.
YUNET_PATH="face_detection_yunet/face_detection_yunet_2023mar.onnx"
YUNET_SHA="8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4"

# The recogniser: turns one aligned face into 128 numbers. 37 MB.
SFACE_PATH="face_recognition_sface/face_recognition_sface_2021dec.onnx"
SFACE_SHA="0ba9fbfa01b5270c96627c4ef784da859931e02f04419c829e83484087c34e79"

fetch() {
    path="$1"
    want="$2"
    name="$(basename "$path")"
    out="${DEST}/${name}"

    if [ -f "$out" ] && [ "$(sha256sum "$out" | cut -d' ' -f1)" = "$want" ]; then
        echo "ok    ${name} is already there"
        return 0
    fi

    echo "..    fetching ${name}"
    tmp="${out}.part"
    if ! curl -fsSL --retry 3 -o "$tmp" "${BASE}/${path}"; then
        rm -f "$tmp"
        echo "fail  could not download ${name}" >&2
        return 1
    fi

    got="$(sha256sum "$tmp" | cut -d' ' -f1)"
    if [ "$got" != "$want" ]; then
        rm -f "$tmp"
        echo "fail  ${name} is not the file this expects" >&2
        echo "      wanted ${want}" >&2
        echo "      got    ${got}" >&2
        return 1
    fi

    # Only once it is known to be the right file. A half-downloaded model left
    # under the real name is a daemon that starts, loads it, and fails on the
    # first face.
    chmod 0644 "$tmp"
    mv "$tmp" "$out"
    echo "ok    ${name}"
}

mkdir -p "$DEST"
fetch "$YUNET_PATH" "$YUNET_SHA"
fetch "$SFACE_PATH" "$SFACE_SHA"

echo
echo "ok    models are in ${DEST}"
echo "      raven-faced loads them at start-up; restart it with:"
echo "        raven-rc restart faced"
