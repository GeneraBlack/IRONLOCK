#!/bin/bash
# ==============================================================================
# Build-Skript für IRONLOCK // RUST-RTS (Server-Image + NethServer 8 Modul-Image)
# Strikte SemVer-Versionierung (kein :latest erlaubt, voll NS8-UI-updatefähig).
# ==============================================================================
set -e

REPODIR="$(cd "$(dirname "$0")" && pwd)"
DEFAULT_VERSION="$(tr -d '[:space:]' < "${REPODIR}/VERSION")"

REPOBASE="${REPOBASE:-${IMAGE_REPO:-reg.bls-isp.net/genera/rust-rts-webserver}}"
REPOBASE="${REPOBASE,,}"
REPONAME="${REPONAME:-rust-rts}"
REPONAME="${REPONAME,,}"
IMAGETAG="${IMAGETAG:-${IMAGE_TAG:-${DEFAULT_VERSION}}}"
IMAGETAG="${IMAGETAG#v}"

if [ "${IMAGETAG,,}" = "latest" ]; then
    echo "FEHLER: Der Tag 'latest' ist verboten! Bitte eine gültige SemVer-Version (z. B. ${DEFAULT_VERSION}) verwenden." >&2
    exit 1
fi

if ! echo "${IMAGETAG}" | grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?$'; then
    echo "FEHLER: Ungültige SemVer-Version '${IMAGETAG}'. Erwartetes Format: X.Y.Z (z. B. 1.0.0)." >&2
    exit 1
fi

SERVER_IMAGE="${SERVER_IMAGE:-${REPOBASE}/rust-rts-server:${IMAGETAG}}"
MODULE_IMAGE="${MODULE_IMAGE:-${REPOBASE}/${REPONAME}:${IMAGETAG}}"

python3 "${REPODIR}/scripts/generate_repodata.py" \
    --version "${IMAGETAG}" \
    --source "${REPOBASE}/${REPONAME}" \
    --server-image "${SERVER_IMAGE}"

if command -v podman >/dev/null 2>&1; then
    CONTAINER_TOOL="podman"
elif command -v buildah >/dev/null 2>&1; then
    CONTAINER_TOOL="buildah"
elif command -v docker >/dev/null 2>&1; then
    CONTAINER_TOOL="docker"
else
    echo "Fehler: Weder podman, buildah noch docker gefunden." >&2
    exit 1
fi

echo "=================================================================="
echo "  [1/2] Baue Application Server Image (${CONTAINER_TOOL})"
echo "        -> ${SERVER_IMAGE}"
echo "=================================================================="
if [ "${CONTAINER_TOOL}" = "buildah" ]; then
    buildah bud \
      --build-arg "MODULE_VERSION=${IMAGETAG}" \
      -f "${REPODIR}/Dockerfile" \
      -t "${SERVER_IMAGE}" \
      "${REPODIR}"
else
    "${CONTAINER_TOOL}" build \
      --build-arg "MODULE_VERSION=${IMAGETAG}" \
      -f "${REPODIR}/Dockerfile" \
      -t "${SERVER_IMAGE}" \
      "${REPODIR}"
fi

echo "=================================================================="
echo "  [2/2] Baue NethServer 8 Rootless Modul-Image (${CONTAINER_TOOL})"
echo "        -> ${MODULE_IMAGE}"
echo "        -> org.nethserver.images=${SERVER_IMAGE}"
echo "=================================================================="
if [ "${CONTAINER_TOOL}" = "buildah" ]; then
    buildah bud \
      --build-arg "MODULE_VERSION=${IMAGETAG}" \
      --build-arg "SERVER_IMAGE=${SERVER_IMAGE}" \
      -f "${REPODIR}/Containerfile.ns8" \
      --label "org.nethserver.images=${SERVER_IMAGE}" \
      --label "org.opencontainers.image.version=${IMAGETAG}" \
      -t "${MODULE_IMAGE}" \
      "${REPODIR}"
else
    "${CONTAINER_TOOL}" build \
      --build-arg "MODULE_VERSION=${IMAGETAG}" \
      --build-arg "SERVER_IMAGE=${SERVER_IMAGE}" \
      -f "${REPODIR}/Containerfile.ns8" \
      --label "org.nethserver.images=${SERVER_IMAGE}" \
      --label "org.opencontainers.image.version=${IMAGETAG}" \
      -t "${MODULE_IMAGE}" \
      "${REPODIR}"
fi

if [ "${PUSH_IMAGES:-0}" = "1" ]; then
    echo "=================================================================="
    echo "  Pushe SemVer-Images (${IMAGETAG}) in Container Registry..."
    echo "=================================================================="
    "${CONTAINER_TOOL}" push "${SERVER_IMAGE}"
    "${CONTAINER_TOOL}" push "${MODULE_IMAGE}"
fi

echo "=================================================================="
echo "  Build (${IMAGETAG}) erfolgreich abgeschlossen!"
echo "  Neuinstallation auf NethServer 8:"
echo "    add-module ${MODULE_IMAGE} 1"
echo "  Update einer bestehenden Instanz (via CLI oder NethServer 8 UI):"
echo "    api-cli run update-module --data '{\"module_url\":\"${MODULE_IMAGE}\",\"instances\":[\"rust-rts1\"],\"force\":false}'"
echo "=================================================================="
