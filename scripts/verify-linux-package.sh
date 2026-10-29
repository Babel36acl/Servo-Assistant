#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  debian)
    docker run --rm -v "$PWD/release-files:/packages:ro" debian:bookworm bash -euo pipefail -c '
      apt-get update
      apt-get install -y /packages/*.deb
      ldd /usr/bin/servo-assistant | tee /tmp/linked-libraries
      ! grep -q "not found" /tmp/linked-libraries
    '
    ;;
  fedora)
    docker run --rm -v "$PWD/release-files:/packages:ro" fedora:latest bash -euo pipefail -c '
      dnf install -y /packages/*.rpm
      ldd /usr/bin/servo-assistant | tee /tmp/linked-libraries
      ! grep -q "not found" /tmp/linked-libraries
    '
    ;;
  arch)
    docker run --rm -v "$PWD/release-files:/packages:ro" archlinux:latest bash -euo pipefail -c '
      pacman-key --init
      pacman-key --populate archlinux
      pacman -Syu --noconfirm gtk3 webkit2gtk-4.1 librsvg
      cp /packages/*.AppImage /tmp/servo.AppImage
      chmod +x /tmp/servo.AppImage
      cd /tmp
      ./servo.AppImage --appimage-extract >/dev/null
      ldd squashfs-root/usr/bin/servo-assistant | tee /tmp/linked-libraries
      ! grep -q "not found" /tmp/linked-libraries
    '
    ;;
  *) printf "Unsupported distribution: %s\n" "$1" >&2; exit 1 ;;
esac
