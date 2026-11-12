import * as crypto from 'node:crypto';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { derivePlayerIdFromToken, verifySecretToken } from './registry';
import {
  WorkshopItemKind,
  WorkshopItemListResponse,
  WorkshopItemMetadata,
  WorkshopPublishMetadataInput,
  WorkshopSortOrder,
} from './types';

export const DEFAULT_MAX_WORKSHOP_UPLOAD_BYTES = 64 * 1024 * 1024; // 64 MiB

const SAFE_ITEM_ID_RE = /^[a-zA-Z0-9][a-zA-Z0-9_-]{1,63}$/;
const SAFE_SEMVER_RE = /^[a-zA-Z0-9][a-zA-Z0-9._+-]{0,31}$/;
const SAFE_SHA256_RE = /^[0-9a-f]{64}$/;
const SAFE_MOD_CONTENT_HASH_RE = /^(?:0x[0-9a-fA-F]{1,64}|[0-9a-fA-F]{4,64}|\d{1,20})$/;

export class WorkshopAuthError extends Error {
  public readonly statusCode: 401 | 403;

  constructor(statusCode: 401 | 403, message: string) {
    super(message);
    this.name = 'WorkshopAuthError';
    this.statusCode = statusCode;
  }
}

export class WorkshopValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'WorkshopValidationError';
  }
}

export class WorkshopPayloadTooLargeError extends Error {
  constructor(message = 'Workshop package exceeds maximum upload size') {
    super(message);
    this.name = 'WorkshopPayloadTooLargeError';
  }
}

function cleanCanonicalWorkshopField(val: unknown): string {
  return String(val ?? '')
    .trim()
    .replace(/[\r\n|]/g, '');
}

/**
 * Canonical HMAC-SHA256 payload shared byte-for-byte with `rts-network/src/lobby_registry.rs`.
 */
export function canonicalWorkshopPublishPayload(input: {
  player_id: string;
  author_name: string;
  timestamp_unix_ms: number;
  id: string;
  kind: string;
  version: string;
  sha256: string;
  mod_content_hash: string;
}): string {
  const ts = Math.max(0, Math.floor(Number(input.timestamp_unix_ms) || 0));
  const lines = [
    'ironlock-workshop-publish-v1',
    `player_id=${cleanCanonicalWorkshopField(input.player_id)}`,
    `author_name=${cleanCanonicalWorkshopField(input.author_name)}`,
    `timestamp_unix_ms=${ts}`,
    `id=${cleanCanonicalWorkshopField(input.id)}`,
    `kind=${cleanCanonicalWorkshopField(input.kind).toLowerCase()}`,
    `version=${cleanCanonicalWorkshopField(input.version)}`,
    `sha256=${cleanCanonicalWorkshopField(input.sha256).toLowerCase()}`,
    `mod_content_hash=${cleanCanonicalWorkshopField(input.mod_content_hash).toLowerCase()}`,
  ];
  return lines.join('\n') + '\n';
}

export function signWorkshopPublish(
  meta: Omit<
    WorkshopPublishMetadataInput,
    'player_id' | 'author_name' | 'public_token' | 'timestamp_unix_ms' | 'signature'
  >,
  profile: { player_id: string; display_name: string; public_token: string },
  timestampUnixMs = Date.now()
): WorkshopPublishMetadataInput {
  const ts = Math.max(0, Math.floor(Number(timestampUnixMs) || 0));
  const canonical = canonicalWorkshopPublishPayload({
    player_id: profile.player_id,
    author_name: profile.display_name,
    timestamp_unix_ms: ts,
    id: meta.id,
    kind: meta.kind,
    version: meta.version,
    sha256: meta.sha256,
    mod_content_hash: meta.mod_content_hash,
  });
  const signature = crypto
    .createHmac('sha256', Buffer.from(profile.public_token.trim(), 'utf-8'))
    .update(canonical, 'utf-8')
    .digest('hex');

  return {
    ...meta,
    player_id: profile.player_id,
    author_name: profile.display_name,
    public_token: profile.public_token,
    timestamp_unix_ms: ts,
    signature,
  };
}

function matchesArchiveContentHash(
  declaredRaw: string,
  archiveContentHash: bigint
): boolean {
  const trimmed = declaredRaw.trim();
  if (!trimmed) return false;
  if (trimmed.startsWith('0x') || trimmed.startsWith('0X')) {
    try {
      return BigInt(trimmed) === archiveContentHash;
    } catch {
      return false;
    }
  }
  if (/^\d+$/.test(trimmed)) {
    try {
      if (BigInt(trimmed) === archiveContentHash) {
        return true;
      }
    } catch {
      // fall through
    }
  }
  if (/^[0-9a-fA-F]+$/.test(trimmed)) {
    try {
      if (BigInt('0x' + trimmed) === archiveContentHash) {
        return true;
      }
    } catch {
      return false;
    }
  }
  return false;
}

function validateRtsModBlob(blob: Buffer, declaredContentHash: string): void {
  if (blob.length >= 8) {
    const magic = blob.subarray(0, 8).toString('ascii');
    if (magic === 'RTSMOD03' || magic === 'RTSMOD02') {
      if (blob.length < 12) {
        throw new WorkshopValidationError(`Truncated ${magic} archive header`);
      }
      const idxLen = blob.readUInt32LE(8);
      if (idxLen === 0 || idxLen > 32 * 1024 * 1024 || 12 + idxLen > blob.length) {
        throw new WorkshopValidationError(`Invalid or truncated ${magic} index block`);
      }
      const indexJsonText = blob.subarray(12, 12 + idxLen).toString('utf-8');
      let parsed: {
        mod_id?: unknown;
        entries?: Array<{
          path?: unknown;
          offset?: unknown;
          compressed_len?: unknown;
          len?: unknown;
        }>;
      };
      try {
        parsed = JSON.parse(indexJsonText);
      } catch {
        throw new WorkshopValidationError(`Corrupt ${magic} JSON index`);
      }
      if (
        !parsed ||
        typeof parsed !== 'object' ||
        typeof parsed.mod_id !== 'string' ||
        !parsed.mod_id.trim() ||
        !Array.isArray(parsed.entries)
      ) {
        throw new WorkshopValidationError(
          `${magic} index missing required mod_id or entries array`
        );
      }
      const payloadLen = blob.length - (12 + idxLen);
      for (const entry of parsed.entries) {
        const offset = Number(entry.offset ?? -1);
        const span = Number(
          magic === 'RTSMOD03' ? entry.compressed_len : entry.len
        );
        if (
          !Number.isInteger(offset) ||
          offset < 0 ||
          !Number.isInteger(span) ||
          span < 0 ||
          offset + span > payloadLen
        ) {
          throw new WorkshopValidationError(
            `Truncated ${magic} entry payload bounds`
          );
        }
      }

      const hashMatch = /"content_hash"\s*:\s*(\d+)/.exec(indexJsonText);
      if (!hashMatch) {
        throw new WorkshopValidationError(
          `${magic} index missing content_hash field`
        );
      }
      const archiveHash = BigInt(hashMatch[1]);
      if (!matchesArchiveContentHash(declaredContentHash, archiveHash)) {
        throw new WorkshopValidationError(
          `Declared mod_content_hash (${declaredContentHash}) does not match archive content_hash (0x${archiveHash.toString(16).padStart(16, '0')})`
        );
      }
      return;
    }
  }

  // Support legacy JSON `.rtsmod` container format as well
  if (blob[0] === 0x7b) {
    const rawText = blob.toString('utf-8');
    let parsed: { mod_id?: unknown; files?: unknown };
    try {
      parsed = JSON.parse(rawText);
    } catch {
      throw new WorkshopValidationError('Invalid legacy JSON .rtsmod container');
    }
    if (
      !parsed ||
      typeof parsed !== 'object' ||
      typeof parsed.mod_id !== 'string' ||
      !parsed.mod_id.trim() ||
      !parsed.files ||
      typeof parsed.files !== 'object'
    ) {
      throw new WorkshopValidationError(
        'Legacy JSON .rtsmod missing mod_id or files'
      );
    }
    const hashMatch = /"content_hash"\s*:\s*(\d+)/.exec(rawText);
    if (hashMatch) {
      const archiveHash = BigInt(hashMatch[1]);
      if (!matchesArchiveContentHash(declaredContentHash, archiveHash)) {
        throw new WorkshopValidationError(
          'Declared mod_content_hash does not match legacy .rtsmod content_hash'
        );
      }
    }
    return;
  }

  throw new WorkshopValidationError(
    'Invalid .rtsmod binary archive: expected RTSMOD03 or RTSMOD02 header'
  );
}

function validateTomlMapBlob(blob: Buffer): void {
  if (blob.includes(0)) {
    throw new WorkshopValidationError(
      'Invalid .toml map payload: binary NUL byte detected'
    );
  }
  const text = blob.toString('utf-8').trim();
  if (text.length < 4 || !text.includes('=')) {
    throw new WorkshopValidationError(
      'Invalid .toml map payload: expected UTF-8 TOML key-value definitions'
    );
  }
}

export interface WorkshopManagerOptions {
  workshopDir?: string;
  dataDir?: string;
  maxUploadBytes?: number;
  apiToken?: string;
}

export interface WorkshopListQuery {
  search?: string;
  tag?: string;
  kind?: string;
  sort?: string;
}

export interface WorkshopDownloadResource {
  metadata: WorkshopItemMetadata;
  sizeBytes: number;
  stream?: fs.ReadStream;
  buffer?: Buffer;
}

export class WorkshopManager {
  public readonly maxUploadBytes: number;
  private readonly storageDir?: string;
  private readonly blobsDir?: string;
  private readonly indexFilePath?: string;
  private readonly configuredApiToken?: string;
  private readonly items = new Map<string, WorkshopItemMetadata>();
  private readonly memoryBlobs = new Map<string, Buffer>();
  private atomicSeq = 0;

  constructor(options: WorkshopManagerOptions = {}) {
    this.maxUploadBytes = Math.max(
      1024,
      options.maxUploadBytes ??
        (Number(process.env.WORKSHOP_MAX_UPLOAD_BYTES) ||
          DEFAULT_MAX_WORKSHOP_UPLOAD_BYTES)
    );
    const tokenCandidate =
      options.apiToken ?? process.env.WORKSHOP_API_TOKEN ?? '';
    this.configuredApiToken =
      tokenCandidate.trim().length > 0 ? tokenCandidate.trim() : undefined;

    const explicitWorkshopDir =
      options.workshopDir?.trim() ||
      (process.env.WORKSHOP_DIR && process.env.WORKSHOP_DIR.trim().length > 0
        ? process.env.WORKSHOP_DIR.trim()
        : undefined);
    const effectiveDataDir =
      options.dataDir?.trim() ||
      (process.env.DATA_DIR && process.env.DATA_DIR.trim().length > 0
        ? process.env.DATA_DIR.trim()
        : undefined);

    const resolvedDir =
      explicitWorkshopDir ??
      (effectiveDataDir ? path.join(effectiveDataDir, 'workshop') : undefined);

    if (resolvedDir) {
      this.storageDir = path.resolve(resolvedDir);
      this.blobsDir = path.join(this.storageDir, 'blobs');
      this.indexFilePath = path.join(this.storageDir, 'index.json');
      this.loadPersistedIndex();
    }
  }

  public get totalItems(): number {
    return this.items.size;
  }

  private loadPersistedIndex(): void {
    if (!this.indexFilePath || !fs.existsSync(this.indexFilePath)) {
      return;
    }
    try {
      const raw = fs.readFileSync(this.indexFilePath, 'utf-8');
      const parsed = JSON.parse(raw) as { items?: WorkshopItemMetadata[] };
      if (parsed && Array.isArray(parsed.items)) {
        for (const item of parsed.items) {
          if (
            item &&
            typeof item.id === 'string' &&
            SAFE_ITEM_ID_RE.test(item.id)
          ) {
            this.items.set(item.id, item);
          }
        }
      }
    } catch {
      // Ignore corrupt index file on cold boot
    }
  }

  private savePersistedIndex(): void {
    if (!this.indexFilePath || !this.storageDir) {
      return;
    }
    fs.mkdirSync(this.storageDir, { recursive: true });
    const payload = JSON.stringify(
      {
        version: 1,
        items: [...this.items.values()],
      },
      null,
      2
    );
    const tmpPath = `${this.indexFilePath}.tmp.${process.pid}.${this.atomicSeq++}`;
    fs.writeFileSync(tmpPath, payload, 'utf-8');
    fs.renameSync(tmpPath, this.indexFilePath);
  }

  private resolveBlobPath(fileName: string): string | undefined {
    if (!this.blobsDir) return undefined;
    const resolvedBlobsRoot = path.resolve(this.blobsDir);
    const rootWithSep = resolvedBlobsRoot.endsWith(path.sep)
      ? resolvedBlobsRoot
      : resolvedBlobsRoot + path.sep;
    const candidate = path.resolve(resolvedBlobsRoot, fileName);
    if (!candidate.startsWith(rootWithSep)) {
      throw new WorkshopValidationError('Unsafe workshop blob file path');
    }
    return candidate;
  }

  private verifyPublishAuth(
    meta: WorkshopPublishMetadataInput,
    bearerOrHeaderToken?: string
  ): { authenticatedPlayerId: string; isApiTokenAdmin: boolean } {
    const providedToken = bearerOrHeaderToken?.trim() ?? '';
    if (providedToken.length > 0 && this.configuredApiToken) {
      if (verifySecretToken(providedToken, this.configuredApiToken)) {
        const cleanPid = String(meta.player_id ?? '').trim() || 'plr-workshop-token';
        return { authenticatedPlayerId: cleanPid, isApiTokenAdmin: true };
      }
      // If no HMAC fields were provided alongside a wrong bearer token, reject immediately
      if (!meta.signature && !meta.public_token) {
        throw new WorkshopAuthError(401, 'Invalid Workshop API bearer token');
      }
    } else if (providedToken.length > 0 && !meta.signature && !meta.public_token) {
      throw new WorkshopAuthError(401, 'Invalid or unconfigured Workshop API token');
    }

    const playerId = String(meta.player_id ?? '').trim();
    const authorName = String(meta.author_name ?? '').trim();
    const publicToken = String(meta.public_token ?? '').trim();
    const sigLower = String(meta.signature ?? '')
      .trim()
      .toLowerCase();
    const ts = Math.max(0, Math.floor(Number(meta.timestamp_unix_ms) || 0));

    if (
      !playerId ||
      !authorName ||
      publicToken.length !== 64 ||
      sigLower.length !== 64 ||
      ts <= 0
    ) {
      throw new WorkshopAuthError(
        401,
        'Missing or malformed cryptographic player_id, public_token, timestamp_unix_ms, or HMAC signature'
      );
    }

    const derivedPid = derivePlayerIdFromToken(publicToken);
    if (!verifySecretToken(playerId, derivedPid)) {
      throw new WorkshopAuthError(
        401,
        'player_id does not match cryptographic public_token derivation'
      );
    }

    const canonical = canonicalWorkshopPublishPayload({
      player_id: playerId,
      author_name: authorName,
      timestamp_unix_ms: ts,
      id: meta.id,
      kind: meta.kind,
      version: meta.version,
      sha256: meta.sha256,
      mod_content_hash: meta.mod_content_hash,
    });
    const expectedSig = crypto
      .createHmac('sha256', Buffer.from(publicToken, 'utf-8'))
      .update(canonical, 'utf-8')
      .digest('hex');

    if (!verifySecretToken(sigLower, expectedSig)) {
      throw new WorkshopAuthError(
        401,
        'HMAC-SHA256 workshop publish signature verification failed'
      );
    }

    return { authenticatedPlayerId: playerId, isApiTokenAdmin: false };
  }

  public publishItem(
    meta: WorkshopPublishMetadataInput,
    blob: Buffer,
    nowMs = Date.now(),
    bearerOrHeaderToken?: string
  ): WorkshopItemMetadata {
    if (!Buffer.isBuffer(blob) || blob.length === 0) {
      throw new WorkshopValidationError('Workshop binary payload must not be empty');
    }
    if (blob.length > this.maxUploadBytes) {
      throw new WorkshopPayloadTooLargeError(
        `Workshop package (${blob.length} bytes) exceeds ${this.maxUploadBytes} byte limit`
      );
    }

    const id = String(meta.id ?? '').trim();
    if (!SAFE_ITEM_ID_RE.test(id)) {
      throw new WorkshopValidationError(
        'Invalid workshop item id: expected 2..64 [a-zA-Z0-9_-] characters'
      );
    }

    const kindRaw = String(meta.kind ?? '')
      .trim()
      .toLowerCase();
    if (kindRaw !== 'mod' && kindRaw !== 'map') {
      throw new WorkshopValidationError(
        'Invalid workshop item kind: expected "mod" or "map"'
      );
    }
    const kind: WorkshopItemKind = kindRaw;

    const title = String(meta.title ?? '')
      .trim()
      .slice(0, 120);
    if (!title) {
      throw new WorkshopValidationError('Workshop item title is required');
    }
    const description = String(meta.description ?? '')
      .trim()
      .slice(0, 2000);
    const authorName = String(meta.author_name ?? '')
      .trim()
      .slice(0, 64);
    if (!authorName) {
      throw new WorkshopValidationError('Workshop author_name is required');
    }

    const version = String(meta.version ?? '').trim();
    if (!SAFE_SEMVER_RE.test(version)) {
      throw new WorkshopValidationError('Invalid workshop item version string');
    }

    const declaredSha256 = String(meta.sha256 ?? '')
      .trim()
      .toLowerCase();
    if (!SAFE_SHA256_RE.test(declaredSha256)) {
      throw new WorkshopValidationError(
        'Invalid sha256 digest: expected 64 lowercase hexadecimal characters'
      );
    }

    const modContentHash = String(meta.mod_content_hash ?? '')
      .trim()
      .toLowerCase();
    if (!SAFE_MOD_CONTENT_HASH_RE.test(modContentHash)) {
      throw new WorkshopValidationError('Invalid or missing mod_content_hash');
    }

    const { authenticatedPlayerId, isApiTokenAdmin } = this.verifyPublishAuth(
      meta,
      bearerOrHeaderToken
    );

    const existing = this.items.get(id);
    if (
      existing &&
      !isApiTokenAdmin &&
      existing.author_player_id !== authenticatedPlayerId
    ) {
      throw new WorkshopAuthError(
        403,
        `Workshop item '${id}' is owned by another publisher`
      );
    }

    const actualSha256 = crypto
      .createHash('sha256')
      .update(blob)
      .digest('hex');
    if (!verifySecretToken(declaredSha256, actualSha256)) {
      throw new WorkshopValidationError(
        `SHA-256 digest mismatch: declared ${declaredSha256}, computed ${actualSha256}`
      );
    }

    if (kind === 'mod') {
      validateRtsModBlob(blob, modContentHash);
    } else {
      validateTomlMapBlob(blob);
    }

    const rawTags = Array.isArray(meta.tags) ? meta.tags : [];
    const normalizedTags: string[] = [];
    const seenTags = new Set<string>();
    for (const rawTag of rawTags) {
      const clean = String(rawTag ?? '')
        .trim()
        .toLowerCase()
        .replace(/[^a-z0-9_+-]/g, '')
        .slice(0, 32);
      if (clean && !seenTags.has(clean) && normalizedTags.length < 16) {
        seenTags.add(clean);
        normalizedTags.push(clean);
      }
    }

    const ext = kind === 'mod' ? '.rtsmod' : '.toml';
    const fileName = `${id}${ext}`;

    if (this.blobsDir) {
      fs.mkdirSync(this.blobsDir, { recursive: true });
      const blobPath = this.resolveBlobPath(fileName)!;
      const tmpBlobPath = `${blobPath}.tmp.${process.pid}.${this.atomicSeq++}`;
      fs.writeFileSync(tmpBlobPath, blob);
      fs.renameSync(tmpBlobPath, blobPath);
      if (existing && existing.file_name !== fileName) {
        const oldPath = this.resolveBlobPath(existing.file_name);
        if (oldPath && fs.existsSync(oldPath)) {
          try {
            fs.unlinkSync(oldPath);
          } catch {
            // ignore
          }
        }
      }
    } else {
      this.memoryBlobs.set(id, Buffer.from(blob));
    }

    const record: WorkshopItemMetadata = {
      id,
      title,
      description,
      author_name: authorName,
      author_player_id: existing ? existing.author_player_id : authenticatedPlayerId,
      kind,
      version,
      tags: normalizedTags,
      sha256: actualSha256,
      mod_content_hash: modContentHash,
      size_bytes: blob.length,
      file_name: fileName,
      downloads: existing ? existing.downloads : 0,
      created_at_unix_ms: existing ? existing.created_at_unix_ms : nowMs,
      updated_at_unix_ms: nowMs,
    };

    this.items.set(id, record);
    this.savePersistedIndex();
    return { ...record, tags: [...record.tags] };
  }

  public listItems(query: WorkshopListQuery = {}): WorkshopItemListResponse {
    const searchLower = query.search?.trim().toLowerCase() ?? '';
    const kindLower = query.kind?.trim().toLowerCase() ?? '';
    const requestedTags = (query.tag ?? '')
      .split(',')
      .map((t) => t.trim().toLowerCase())
      .filter(Boolean);
    const sortRaw = (query.sort?.trim().toLowerCase() ?? 'downloads') as string;

    const filtered: WorkshopItemMetadata[] = [];
    for (const item of this.items.values()) {
      if (kindLower === 'mod' || kindLower === 'map') {
        if (item.kind !== kindLower) continue;
      } else if (kindLower.length > 0) {
        continue;
      }

      if (requestedTags.length > 0) {
        const hasAllTags = requestedTags.every((rt) =>
          item.tags.some((t) => t.toLowerCase() === rt)
        );
        if (!hasAllTags) continue;
      }

      if (searchLower.length > 0) {
        const haystack =
          `${item.id} ${item.title} ${item.description} ${item.author_name} ${item.tags.join(' ')}`.toLowerCase();
        if (!haystack.includes(searchLower)) continue;
      }

      filtered.push({ ...item, tags: [...item.tags] });
    }

    const sortMode: WorkshopSortOrder =
      sortRaw === 'newest' || sortRaw === 'created'
        ? 'newest'
        : sortRaw === 'updated' || sortRaw === 'recent'
          ? 'updated'
          : sortRaw === 'title' || sortRaw === 'name'
            ? 'title'
            : 'downloads';

    filtered.sort((a, b) => {
      if (sortMode === 'newest') {
        if (b.created_at_unix_ms !== a.created_at_unix_ms) {
          return b.created_at_unix_ms - a.created_at_unix_ms;
        }
      } else if (sortMode === 'updated') {
        if (b.updated_at_unix_ms !== a.updated_at_unix_ms) {
          return b.updated_at_unix_ms - a.updated_at_unix_ms;
        }
      } else if (sortMode === 'title') {
        const cmp = a.title.localeCompare(b.title);
        if (cmp !== 0) return cmp;
      } else {
        if (b.downloads !== a.downloads) {
          return b.downloads - a.downloads;
        }
        if (b.updated_at_unix_ms !== a.updated_at_unix_ms) {
          return b.updated_at_unix_ms - a.updated_at_unix_ms;
        }
      }
      return a.id.localeCompare(b.id);
    });

    return {
      items: filtered,
      total: filtered.length,
    };
  }

  public getItem(id: string): WorkshopItemMetadata | null {
    const cleanId = id.trim();
    if (!SAFE_ITEM_ID_RE.test(cleanId)) return null;
    const found = this.items.get(cleanId);
    return found ? { ...found, tags: [...found.tags] } : null;
  }

  public openDownload(id: string): WorkshopDownloadResource | null {
    const cleanId = id.trim();
    if (!SAFE_ITEM_ID_RE.test(cleanId)) {
      return null;
    }
    const item = this.items.get(cleanId);
    if (!item) {
      return null;
    }

    if (this.blobsDir) {
      const blobPath = this.resolveBlobPath(item.file_name);
      if (!blobPath || !fs.existsSync(blobPath)) {
        return null;
      }
      const stat = fs.statSync(blobPath);
      item.downloads += 1;
      this.savePersistedIndex();
      return {
        metadata: { ...item, tags: [...item.tags] },
        sizeBytes: stat.size,
        stream: fs.createReadStream(blobPath),
      };
    }

    const memBuf = this.memoryBlobs.get(cleanId);
    if (!memBuf) {
      return null;
    }
    item.downloads += 1;
    return {
      metadata: { ...item, tags: [...item.tags] },
      sizeBytes: memBuf.length,
      buffer: memBuf,
    };
  }
}
