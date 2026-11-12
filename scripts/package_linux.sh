#!/usr/bin/env bash
set -euo pipefail

# Usage: ./scripts/package_linux.sh <deb|rpm> <pkg_name> <version> <source_dir> <output_dir>
FORMAT="${1:?Format (deb|rpm) required}"
PKG_NAME="${2:?Package name required}"
VERSION="${3:?Version required}"
SRC_DIR="${4:?Source directory required}"
OUT_DIR="${5:?Output directory required}"

mkdir -p "$OUT_DIR"
ABS_SRC="$(cd "$SRC_DIR" && pwd)"
ABS_OUT="$(cd "$OUT_DIR" && pwd)"

if [[ "$FORMAT" == "deb" ]]; then
    STAGE_DIR="$(mktemp -d)"
    trap 'rm -rf "$STAGE_DIR"' EXIT

    mkdir -p "$STAGE_DIR/DEBIAN"
    mkdir -p "$STAGE_DIR/opt/$PKG_NAME"
    mkdir -p "$STAGE_DIR/usr/bin"
    mkdir -p "$STAGE_DIR/usr/share/applications"

    cp -a "$ABS_SRC/." "$STAGE_DIR/opt/$PKG_NAME/"

    cat > "$STAGE_DIR/usr/bin/$PKG_NAME" <<EOF
#!/usr/bin/env bash
cd "/opt/$PKG_NAME"
exec "./rts-client" --player-mode "\$@"
EOF
    chmod 0755 "$STAGE_DIR/usr/bin/$PKG_NAME"

    cat > "$STAGE_DIR/usr/share/applications/$PKG_NAME.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=IRONLOCK ($PKG_NAME)
Comment=IRONLOCK 3D Real-Time Strategy Engine
Exec=/usr/bin/$PKG_NAME
Terminal=false
Categories=Game;StrategyGame;
EOF

    INSTALLED_KB="$(du -sk "$STAGE_DIR/opt/$PKG_NAME" | awk '{print $1}')"
    cat > "$STAGE_DIR/DEBIAN/control" <<EOF
Package: $PKG_NAME
Version: $VERSION
Section: games
Priority: optional
Architecture: amd64
Installed-Size: $INSTALLED_KB
Depends: libc6 (>= 2.31), libasound2, libx11-6, libxi6, libxcursor1, libxrandr2, libxinerama1, libxkbcommon0, libwayland-client0
Maintainer: GeneraBlack <meier.Alexander1@googlemail.com>
Homepage: https://github.com/GeneraBlack/IRONLOCK
Description: IRONLOCK 3D RTS Engine & Total Conversion ($PKG_NAME)
 Non-commercial 3D real-time strategy engine and cooked mod archive.
EOF

    DEB_FILE="$ABS_OUT/${PKG_NAME}_${VERSION}_amd64.deb"
    dpkg-deb --build --root-owner-group "$STAGE_DIR" "$DEB_FILE"
    echo "[DEB] Built and verified: $DEB_FILE"
    dpkg-deb --info "$DEB_FILE"

elif [[ "$FORMAT" == "rpm" ]]; then
    RPM_TOP="$(mktemp -d)"
    trap 'rm -rf "$RPM_TOP"' EXIT

    mkdir -p "$RPM_TOP"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

    cat > "$RPM_TOP/SPECS/$PKG_NAME.spec" <<EOF
Name:           $PKG_NAME
Version:        $VERSION
Release:        1%{?dist}
Summary:        IRONLOCK 3D RTS Engine & Total Conversion ($PKG_NAME)
License:        MIT OR Apache-2.0
URL:            https://github.com/GeneraBlack/IRONLOCK
BuildArch:      x86_64
AutoReqProv:    no
Requires:       glibc, alsa-lib, libX11, libXi, libXcursor, libXrandr, libXinerama, libxkbcommon, libwayland-client

%description
Non-commercial 3D real-time strategy engine and cooked mod archive ($PKG_NAME).

%install
mkdir -p %{buildroot}/opt/$PKG_NAME
mkdir -p %{buildroot}/usr/bin
mkdir -p %{buildroot}/usr/share/applications
cp -a "$ABS_SRC/." %{buildroot}/opt/$PKG_NAME/

cat > %{buildroot}/usr/bin/$PKG_NAME <<'LAUNCHER'
#!/usr/bin/env bash
cd "/opt/$PKG_NAME"
exec "./rts-client" --player-mode "\$@"
LAUNCHER
chmod 0755 %{buildroot}/usr/bin/$PKG_NAME

cat > %{buildroot}/usr/share/applications/$PKG_NAME.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=IRONLOCK ($PKG_NAME)
Comment=IRONLOCK 3D Real-Time Strategy Engine
Exec=/usr/bin/$PKG_NAME
Terminal=false
Categories=Game;StrategyGame;
DESKTOP

%files
/opt/$PKG_NAME
/usr/bin/$PKG_NAME
/usr/share/applications/$PKG_NAME.desktop
EOF

    rpmbuild --define "_topdir $RPM_TOP" --define "_binary_payload w2.gzdio" -bb "$RPM_TOP/SPECS/$PKG_NAME.spec"
    find "$RPM_TOP/RPMS" -name "*.rpm" -exec cp -v {} "$ABS_OUT/" \;
    for rpm_file in "$ABS_OUT"/${PKG_NAME}-${VERSION}*.rpm; do
        echo "[RPM] Built and verified: $rpm_file"
        rpm -qip "$rpm_file"
    done
else
    echo "Unknown format: $FORMAT (expected deb or rpm)" >&2
    exit 1
fi
