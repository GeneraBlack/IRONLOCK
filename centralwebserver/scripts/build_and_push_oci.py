#!/usr/bin/env python3
"""
Rootless, zero-privilege OCI / Docker Registry v2 image builder & pusher.
Builds and pushes:
  1. Server image: <repo_base>/rust-rts-server:<semver>
     (based on docker.io/library/node:22.14.0-alpine3.21 + /app layer)
  2. NethServer 8 module image: <repo_base>/rust-rts:<semver> and <repo_base>:<semver>
     (FROM scratch + /imageroot + /ui with NS8 labels)

Requires NO privileged container capabilities (no unshare, mount, or chroot)
and NO GitLab artifact uploader. Strictly forbids ':latest'.
"""

import argparse
import base64
import gzip
import hashlib
import io
import json
import os
import re
import sys
import tarfile
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

SEMVER_RE = re.compile(
    r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z.-]+))?$"
)

BASE_NODE_REPO = "library/node"
BASE_NODE_TAG = "22.14.0-alpine3.21"
MANIFEST_V2_MIME = "application/vnd.docker.distribution.manifest.v2+json"
MANIFEST_LIST_MIME = "application/vnd.docker.distribution.manifest.list.v2+json"
OCI_INDEX_MIME = "application/vnd.oci.image.index.v1+json"
OCI_MANIFEST_MIME = "application/vnd.oci.image.manifest.v1+json"
CONFIG_V1_MIME = "application/vnd.docker.container.image.v1+json"
LAYER_TAR_GZ_MIME = "application/vnd.docker.image.rootfs.diff.tar.gzip"


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def parse_www_authenticate(header: str) -> dict[str, str]:
    result: dict[str, str] = {}
    for match in re.finditer(r'([a-zA-Z0-9_]+)="([^"]*)"', header):
        result[match.group(1)] = match.group(2)
    return result


class RegistryClient:
    def __init__(self, registry_host: str, username: str | None = None, password: str | None = None):
        self.registry_host = registry_host.rstrip("/")
        if not self.registry_host.startswith("http"):
            self.base_url = f"https://{self.registry_host}"
        else:
            self.base_url = self.registry_host
        self.username = username
        self.password = password
        self.tokens: dict[str, str] = {}

    def _get_bearer_token(self, www_auth: str, scope_hint: str) -> str:
        params = parse_www_authenticate(www_auth)
        realm = params.get("realm")
        if not realm:
            raise RuntimeError(f"Missing realm in Www-Authenticate: {www_auth}")
        query: list[tuple[str, str]] = []
        if "service" in params:
            query.append(("service", params["service"]))
        scope = params.get("scope") or scope_hint
        if scope:
            query.append(("scope", scope))

        token_url = f"{realm}?{urllib.parse.urlencode(query)}"
        req = urllib.request.Request(token_url, method="GET")
        if self.username and self.password:
            creds = base64.b64encode(f"{self.username}:{self.password}".encode("utf-8")).decode("ascii")
            req.add_header("Authorization", f"Basic {creds}")

        with urllib.request.urlopen(req, timeout=30) as resp:
            payload = json.loads(resp.read().decode("utf-8"))
        token = payload.get("token") or payload.get("access_token")
        if not token:
            raise RuntimeError(f"No token in auth response from {realm}")
        return token

    def request(
        self,
        method: str,
        path_or_url: str,
        repo_scope: str,
        data: bytes | None = None,
        headers: dict[str, str] | None = None,
        expected_statuses: tuple[int, ...] = (200, 201, 202),
    ) -> tuple[int, dict[str, str], bytes]:
        url = path_or_url if path_or_url.startswith("http") else f"{self.base_url}{path_or_url}"
        hdrs = dict(headers or {})
        if repo_scope in self.tokens:
            hdrs["Authorization"] = f"Bearer {self.tokens[repo_scope]}"

        for attempt in range(2):
            req = urllib.request.Request(url, data=data, headers=hdrs, method=method)
            try:
                with urllib.request.urlopen(req, timeout=120) as resp:
                    status = resp.status
                    resp_headers = {k.lower(): v for k, v in resp.headers.items()}
                    body = resp.read() if method != "HEAD" else b""
                    return status, resp_headers, body
            except urllib.error.HTTPError as err:
                if err.code == 401 and attempt == 0:
                    www_auth = err.headers.get("Www-Authenticate", "")
                    if "Bearer" in www_auth:
                        token = self._get_bearer_token(www_auth, repo_scope)
                        self.tokens[repo_scope] = token
                        hdrs["Authorization"] = f"Bearer {token}"
                        continue
                    elif self.username and self.password:
                        creds = base64.b64encode(
                            f"{self.username}:{self.password}".encode("utf-8")
                        ).decode("ascii")
                        hdrs["Authorization"] = f"Basic {creds}"
                        continue
                if err.code in expected_statuses:
                    resp_headers = {k.lower(): v for k, v in err.headers.items()}
                    return err.code, resp_headers, b""
                err_body = err.read().decode("utf-8", errors="replace")
                raise RuntimeError(
                    f"Registry HTTP {err.code} on {method} {url}: {err_body}"
                ) from err
        raise RuntimeError(f"Failed request {method} {url}")

    def blob_exists(self, repo: str, digest: str) -> bool:
        scope = f"repository:{repo}:pull,push"
        status, _, _ = self.request(
            "HEAD",
            f"/v2/{repo}/blobs/{digest}",
            repo_scope=scope,
            expected_statuses=(200, 404),
        )
        return status == 200

    def upload_blob(self, repo: str, data: bytes, digest: str) -> None:
        if self.blob_exists(repo, digest):
            print(f"  [Cache] Blob {digest[:19]}... existiert bereits in {repo}")
            return

        scope = f"repository:{repo}:pull,push"
        status, hdrs, _ = self.request(
            "POST",
            f"/v2/{repo}/blobs/uploads/",
            repo_scope=scope,
            data=b"",
            headers={"Content-Length": "0"},
            expected_statuses=(202,),
        )
        location = hdrs.get("location")
        if not location:
            raise RuntimeError(f"Missing Location header starting blob upload for {repo}")

        sep = "&" if "?" in location else "?"
        put_url = f"{location}{sep}digest={urllib.parse.quote(digest, safe=':')}"
        self.request(
            "PUT",
            put_url,
            repo_scope=scope,
            data=data,
            headers={
                "Content-Type": "application/octet-stream",
                "Content-Length": str(len(data)),
            },
            expected_statuses=(201,),
        )
        print(f"  [Push] Blob {digest[:19]}... ({len(data)} Bytes) -> {repo}")

    def put_manifest(self, repo: str, tag: str, manifest_bytes: bytes, content_type: str = MANIFEST_V2_MIME) -> None:
        scope = f"repository:{repo}:pull,push"
        self.request(
            "PUT",
            f"/v2/{repo}/manifests/{tag}",
            repo_scope=scope,
            data=manifest_bytes,
            headers={
                "Content-Type": content_type,
                "Content-Length": str(len(manifest_bytes)),
            },
            expected_statuses=(201, 200),
        )
        print(f"  [Push] Manifest {repo}:{tag} erfolgreich veröffentlicht!")


def add_dir_to_tar(
    tf: tarfile.TarFile,
    src_dir: Path,
    arc_prefix: str,
    executable_predicate=None,
    strip_crlf: bool = False,
) -> None:
    arc_prefix = arc_prefix.strip("/")
    dir_info = tarfile.TarInfo(name=arc_prefix)
    dir_info.type = tarfile.DIRTYPE
    dir_info.mode = 0o755
    dir_info.uid = 0
    dir_info.gid = 0
    dir_info.mtime = 1700000000
    tf.addfile(dir_info)

    for item in sorted(src_dir.rglob("*")):
        if "__pycache__" in item.parts or item.suffix == ".pyc":
            continue
        rel = item.relative_to(src_dir).as_posix()
        arcname = f"{arc_prefix}/{rel}"
        info = tarfile.TarInfo(name=arcname)
        info.uid = 0
        info.gid = 0
        info.mtime = 1700000000

        if item.is_dir():
            info.type = tarfile.DIRTYPE
            info.mode = 0o755
            tf.addfile(info)
        elif item.is_file():
            raw = item.read_bytes()
            if strip_crlf and b"\r\n" in raw and item.suffix.lower() not in (".png", ".ico", ".jpg", ".jpeg", ".webp", ".glb", ".woff", ".woff2"):
                raw = raw.replace(b"\r\n", b"\n")
            info.type = tarfile.REGTYPE
            info.size = len(raw)
            is_exec = bool(executable_predicate and executable_predicate(rel))
            info.mode = 0o755 if is_exec else 0o644
            tf.addfile(info, io.BytesIO(raw))


def add_file_to_tar(tf: tarfile.TarFile, src_file: Path, arcname: str, mode: int = 0o644) -> None:
    raw = src_file.read_bytes()
    info = tarfile.TarInfo(name=arcname.lstrip("/"))
    info.type = tarfile.REGTYPE
    info.size = len(raw)
    info.mode = mode
    info.uid = 0
    info.gid = 0
    info.mtime = 1700000000
    tf.addfile(info, io.BytesIO(raw))


def create_layer_blobs(build_tar_fn) -> tuple[str, str, bytes]:
    raw_buf = io.BytesIO()
    with tarfile.open(fileobj=raw_buf, mode="w", format=tarfile.PAX_FORMAT) as tf:
        build_tar_fn(tf)
    uncompressed_bytes = raw_buf.getvalue()
    diff_id = f"sha256:{sha256_hex(uncompressed_bytes)}"

    gz_buf = io.BytesIO()
    with gzip.GzipFile(filename="", mode="wb", fileobj=gz_buf, mtime=0) as gz:
        gz.write(uncompressed_bytes)
    compressed_bytes = gz_buf.getvalue()
    layer_digest = f"sha256:{sha256_hex(compressed_bytes)}"
    return diff_id, layer_digest, compressed_bytes


def build_ns8_module_image(
    root: Path,
    version: str,
    server_image_ref: str,
) -> tuple[bytes, str, bytes, str, bytes]:
    def is_imageroot_exec(rel_path: str) -> bool:
        parts = rel_path.split("/")
        if len(parts) >= 2 and parts[0] in ("bin", "update-module.d", "events"):
            return True
        if len(parts) >= 3 and parts[0] == "actions" and not parts[-1].endswith(".json"):
            return True
        return False

    def populate_module_tar(tf: tarfile.TarFile) -> None:
        add_dir_to_tar(
            tf,
            root / "imageroot",
            "imageroot",
            executable_predicate=is_imageroot_exec,
            strip_crlf=True,
        )
        add_dir_to_tar(
            tf,
            root / "ui" / "dist",
            "ui",
            executable_predicate=None,
            strip_crlf=False,
        )

    diff_id, layer_digest, layer_gz = create_layer_blobs(populate_module_tar)

    config_obj = {
        "created": "2026-11-07T00:00:00Z",
        "architecture": "amd64",
        "os": "linux",
        "config": {
            "Entrypoint": ["/"],
            "Labels": {
                "org.nethserver.authorizations": "traefik@node:routeadm",
                "org.nethserver.tcp-ports-demand": "1",
                "org.nethserver.rootfull": "0",
                "org.nethserver.min-core": "3.0.0",
                "org.nethserver.volumes": "rust-rts-data",
                "org.nethserver.images": server_image_ref,
                "org.opencontainers.image.title": "IRONLOCK // RUST-RTS",
                "org.opencontainers.image.version": version,
            },
        },
        "rootfs": {
            "type": "layers",
            "diff_ids": [diff_id],
        },
        "history": [
            {
                "created": "2026-11-07T00:00:00Z",
                "created_by": "IRONLOCK // RUST-RTS NS8 OCI Builder",
            }
        ],
    }
    config_bytes = json.dumps(config_obj, separators=(",", ":")).encode("utf-8")
    config_digest = f"sha256:{sha256_hex(config_bytes)}"

    manifest_obj = {
        "schemaVersion": 2,
        "mediaType": MANIFEST_V2_MIME,
        "config": {
            "mediaType": CONFIG_V1_MIME,
            "size": len(config_bytes),
            "digest": config_digest,
        },
        "layers": [
            {
                "mediaType": LAYER_TAR_GZ_MIME,
                "size": len(layer_gz),
                "digest": layer_digest,
            }
        ],
    }
    manifest_bytes = json.dumps(manifest_obj, indent=2).encode("utf-8")
    return layer_gz, layer_digest, config_bytes, config_digest, manifest_bytes


def build_and_push_server_image(
    root: Path,
    version: str,
    target_client: RegistryClient,
    target_repo: str,
) -> None:
    dockerhub = RegistryClient("https://registry-1.docker.io")
    scope = f"repository:{BASE_NODE_REPO}:pull"
    accept_hdr = ", ".join([MANIFEST_LIST_MIME, OCI_INDEX_MIME, MANIFEST_V2_MIME, OCI_MANIFEST_MIME])

    _, _, idx_bytes = dockerhub.request(
        "GET",
        f"/v2/{BASE_NODE_REPO}/manifests/{BASE_NODE_TAG}",
        repo_scope=scope,
        headers={"Accept": accept_hdr},
    )
    idx_doc = json.loads(idx_bytes.decode("utf-8"))

    if "manifests" in idx_doc:
        amd64_digest = None
        for m in idx_doc["manifests"]:
            plat = m.get("platform", {})
            if plat.get("architecture") == "amd64" and plat.get("os") == "linux":
                amd64_digest = m["digest"]
                break
        if not amd64_digest:
            raise RuntimeError(f"Could not find linux/amd64 manifest for {BASE_NODE_REPO}:{BASE_NODE_TAG}")
        _, _, man_bytes = dockerhub.request(
            "GET",
            f"/v2/{BASE_NODE_REPO}/manifests/{amd64_digest}",
            repo_scope=scope,
            headers={"Accept": f"{MANIFEST_V2_MIME}, {OCI_MANIFEST_MIME}"},
        )
        base_manifest = json.loads(man_bytes.decode("utf-8"))
    else:
        base_manifest = idx_doc

    base_cfg_digest = base_manifest["config"]["digest"]
    _, _, base_cfg_bytes = dockerhub.request(
        "GET",
        f"/v2/{BASE_NODE_REPO}/blobs/{base_cfg_digest}",
        repo_scope=scope,
    )
    base_config = json.loads(base_cfg_bytes.decode("utf-8"))

    # Copy base layers to target registry if not already cached
    layers_list = []
    for layer in base_manifest["layers"]:
        l_digest = layer["digest"]
        l_size = layer["size"]
        if not target_client.blob_exists(target_repo, l_digest):
            print(f"  [Base-Layer] Lade {l_digest[:19]}... ({l_size} Bytes) von Docker Hub...")
            _, _, l_bytes = dockerhub.request(
                "GET",
                f"/v2/{BASE_NODE_REPO}/blobs/{l_digest}",
                repo_scope=scope,
            )
            target_client.upload_blob(target_repo, l_bytes, l_digest)
        else:
            print(f"  [Cache] Base-Layer {l_digest[:19]}... bereits in {target_repo}")
        layers_list.append(
            {
                "mediaType": LAYER_TAR_GZ_MIME,
                "size": l_size,
                "digest": l_digest,
            }
        )

    # Create application layer (/app)
    def populate_app_tar(tf: tarfile.TarFile) -> None:
        app_dir = tarfile.TarInfo(name="app")
        app_dir.type = tarfile.DIRTYPE
        app_dir.mode = 0o755
        app_dir.mtime = 1700000000
        tf.addfile(app_dir)

        add_file_to_tar(tf, root / "package.json", "app/package.json", 0o644)
        add_dir_to_tar(tf, root / "dist", "app/dist")
        add_dir_to_tar(tf, root / "public", "app/public")

    app_diff_id, app_layer_digest, app_layer_gz = create_layer_blobs(populate_app_tar)
    target_client.upload_blob(target_repo, app_layer_gz, app_layer_digest)
    layers_list.append(
        {
            "mediaType": LAYER_TAR_GZ_MIME,
            "size": len(app_layer_gz),
            "digest": app_layer_digest,
        }
    )

    # Update OCI image config
    cfg_section = base_config.setdefault("config", {})
    env_list = list(cfg_section.get("Env", []))
    env_overrides = {
        "NODE_ENV": "production",
        "HOST": "0.0.0.0",
        "PORT": "8080",
        "PUBLIC_DOMAIN": "rust-rts.bls-isp.net",
        "PUBLIC_HOST": "rust-rts.bls-isp.net",
        "NODE_ID": "ns8-bls-isp-01",
        "TTL_SECS": "45",
        "RELAY_PORT_START": "30100",
        "RELAY_PORT_END": "30139",
    }
    filtered_env = [e for e in env_list if e.split("=", 1)[0] not in env_overrides]
    for k, v in env_overrides.items():
        filtered_env.append(f"{k}={v}")

    cfg_section["Env"] = filtered_env
    cfg_section["WorkingDir"] = "/app"
    cfg_section["Cmd"] = ["node", "dist/src/index.js"]
    cfg_section["ExposedPorts"] = {"8080/tcp": {}}
    labels = dict(cfg_section.get("Labels") or {})
    labels["org.opencontainers.image.title"] = "IRONLOCK // RUST-RTS Server"
    labels["org.opencontainers.image.version"] = version
    cfg_section["Labels"] = labels

    base_config["rootfs"]["diff_ids"].append(app_diff_id)
    base_config.setdefault("history", []).append(
        {
            "created": "2026-11-07T00:00:00Z",
            "created_by": f"IRONLOCK // RUST-RTS Server {version}",
        }
    )

    new_cfg_bytes = json.dumps(base_config, separators=(",", ":")).encode("utf-8")
    new_cfg_digest = f"sha256:{sha256_hex(new_cfg_bytes)}"
    target_client.upload_blob(target_repo, new_cfg_bytes, new_cfg_digest)

    new_manifest = {
        "schemaVersion": 2,
        "mediaType": MANIFEST_V2_MIME,
        "config": {
            "mediaType": CONFIG_V1_MIME,
            "size": len(new_cfg_bytes),
            "digest": new_cfg_digest,
        },
        "layers": layers_list,
    }
    new_manifest_bytes = json.dumps(new_manifest, indent=2).encode("utf-8")
    target_client.put_manifest(target_repo, version, new_manifest_bytes, MANIFEST_V2_MIME)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True, help="SemVer version tag (e.g. 1.0.5)")
    parser.add_argument("--registry", default="reg.bls-isp.net", help="Target container registry host")
    parser.add_argument(
        "--project-path",
        default="genera/rust-rts-webserver",
        help="Repository namespace/project path inside registry",
    )
    parser.add_argument("--dry-run", action="store_true", help="Only build layers & manifests locally without pushing")
    args = parser.parse_args()

    version = args.version.strip().lstrip("v")
    if version.lower() == "latest" or not SEMVER_RE.match(version):
        print(f"FEHLER: Ungültige SemVer-Version '{version}' ('latest' ist verboten)!", file=sys.stderr)
        return 1

    root = Path(__file__).resolve().parent.parent
    registry_host = args.registry.strip().lower().removeprefix("https://").removeprefix("http://").rstrip("/")
    project_path = args.project_path.strip().lower().strip("/")
    if project_path.startswith(f"{registry_host}/"):
        project_path = project_path[len(registry_host) + 1 :]

    server_repo = f"{project_path}/rust-rts-server"
    module_repo = f"{project_path}/rust-rts"
    root_module_repo = project_path
    server_image_ref = f"{registry_host}/{server_repo}:{version}"

    print("==================================================================")
    print(f"  Baue NS8-Modul-Image im Speicher: {registry_host}/{module_repo}:{version}")
    print(f"  -> org.nethserver.images = {server_image_ref}")
    print("==================================================================")
    mod_layer_gz, mod_layer_digest, mod_cfg_bytes, mod_cfg_digest, mod_manifest_bytes = (
        build_ns8_module_image(root, version, server_image_ref)
    )
    print(f"  [OK] NS8-Modul-Layer gebaut: {mod_layer_digest[:19]}... ({len(mod_layer_gz)} Bytes)")

    if args.dry_run:
        print("  [Dry-Run] Kein Registry-Push ausgeführt.")
        return 0

    user = os.environ.get("CI_REGISTRY_USER") or os.environ.get("CUSTOM_REGISTRY_USER")
    password = os.environ.get("CI_REGISTRY_PASSWORD") or os.environ.get("CUSTOM_REGISTRY_PASSWORD")
    if not user or not password:
        print("HINWEIS: Keine Registry-Credentials (CI_REGISTRY_USER / CI_REGISTRY_PASSWORD) gesetzt — überspringe Push.")
        return 0

    client = RegistryClient(registry_host, user, password)

    print("==================================================================")
    print(f"  [1/3] Baue & pushe Server-Image -> {server_image_ref}")
    print("==================================================================")
    build_and_push_server_image(root, version, client, server_repo)

    print("==================================================================")
    print(f"  [2/3] Pushe NS8-Modul-Image -> {registry_host}/{module_repo}:{version}")
    print("==================================================================")
    client.upload_blob(module_repo, mod_layer_gz, mod_layer_digest)
    client.upload_blob(module_repo, mod_cfg_bytes, mod_cfg_digest)
    client.put_manifest(module_repo, version, mod_manifest_bytes, MANIFEST_V2_MIME)

    print("==================================================================")
    print(f"  [3/3] Pushe NS8-Modul-Image -> {registry_host}/{root_module_repo}:{version}")
    print("==================================================================")
    client.upload_blob(root_module_repo, mod_layer_gz, mod_layer_digest)
    client.upload_blob(root_module_repo, mod_cfg_bytes, mod_cfg_digest)
    client.put_manifest(root_module_repo, version, mod_manifest_bytes, MANIFEST_V2_MIME)

    print("==================================================================")
    print(f"  ALLE SEMVER-IMAGES ({version}) ERFOLGREICH VERÖFFENTLICHT!")
    print(f"  Installation auf NethServer 8:")
    print(f"    add-module {registry_host}/{module_repo}:{version} 1")
    print(f"  Update auf NethServer 8 (via NS8 UI oder CLI):")
    print(f"    api-cli run update-module --data '{{\"module_url\":\"{registry_host}/{module_repo}:{version}\",\"instances\":[\"rust-rts1\"],\"force\":false}}'")
    print("==================================================================")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
