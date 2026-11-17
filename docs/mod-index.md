# Mod index

The mod browser reads one or more indexes: JSON documents that list mods, their versions and where to download them.
This page is the format (version 1) and the HTTP API a server (for example `castanea-backend`) may offer later.
Code: `crates/core/src/index/`. A complete example: [examples/index.json](examples/index.json) (the tests parse it).

## Where indexes come from

Settings → Mod indexes holds a list of locations, read in order:

- an `https://` URL to one `index.json` (a file in a repo, a CDN, or the backend's `GET /v1/mods/index.json`);
- a local path or `file://` URL (an index on disk, for testing a mod before publishing it).

The first index that lists a mod id wins. A remote index is cached in `cache/index/`; the launcher sends `If-None-Match`
with the cached ETag and uses the cached copy when it is offline or the server cannot be reached (the browser says so).

## index.json

```json
{
  "format": 1,
  "name": "castanea mods",
  "updated": "2026-11-17T12:00:00Z",
  "news": [{ "title": "...", "date": "2026-11-17", "body": "...", "url": "https://..." }],
  "mods": [ { "id": "amber", "name": "Amber", "versions": [ { "version": "0.2.0", "url": "https://...", "size": 4096, "sha256": "..." } ] } ],
  "signature": null
}
```

| Key | Required | Meaning |
|---|---|---|
| `format` | yes | `1`. Another number is refused, so a later format can change anything |
| `name` | no | Shown in the browser |
| `updated` | no | RFC 3339 time of the last change |
| `news` | no | Items for the news area of the Play tab: `title`, `date`, `body`, `url` |
| `mods` | yes | The mods, see below. One id at most once |
| `signature` | no | Planned: Ed25519 signature, see [Signatures](#signatures). Ignored today |

Unknown keys are ignored, so servers may add fields.

### A mod

| Key | Required | Meaning |
|---|---|---|
| `id` | yes | The `id` of its `mod.json`: lowercase letters, digits and `_` |
| `name` | yes | Display name |
| `summary` | no | One line for the list |
| `description` | no | Plain text for the details page; blank lines separate paragraphs |
| `authors` | no | List of names |
| `license` | no | SPDX id, such as `MIT` |
| `tags` | no | List of words for the tag filter (`blocks`, `sky`, `machines`, `scripting`, `example` ...) |
| `homepage`, `source` | no | `https://` links |
| `icon` | no | `https://` URL of a square PNG, at most 4 MiB (256 px is plenty) |
| `screenshots` | no | `https://` URLs of PNG or JPEG pictures, at most 4 MiB each |
| `side` | no | `both` (default), `server` or `client`, as in `mod.json` |
| `has_code` | no | `true` when the package carries Wasm code; the launcher warns before enabling it |
| `versions` | yes | The versions, any order (the launcher sorts newest first by semver) |

### A version

| Key | Required | Meaning |
|---|---|---|
| `version` | yes | Must equal `version` in the archive's `mod.json`, or the install is refused |
| `game` | no | A semver range of game versions, such as `>=0.2.0, <0.4.0`. Missing means "unknown": shown, not blocked |
| `depends` | no | Object of mod id to a version range (`"*"` for any). The browser offers to install missing ones |
| `published` | no | Date or RFC 3339 time |
| `url` | yes | `https://` URL of the zip. Redirects are followed, but only to `https://` |
| `size` | yes | Bytes of the zip; a different length fails the download. At most 64 MiB |
| `sha256` | yes | Lower-case hex SHA-256 of the zip (`sha256:` prefix allowed) |
| `changelog` | no | Plain text |

The archive is the game's package format: a zip with `mod.json` at the top (`castanea-mod pack` makes one). The
`id` and `version` inside must match the index entry.

## What the launcher does on install

1. Downloads the zip to `cache/downloads/<sha256>.part` over HTTPS, counting bytes against `size` and the 64 MiB limit.
2. Checks the SHA-256; a mismatch deletes the file and reports both hashes.
3. Opens the zip: every entry name must be relative with no `..`, no links, at most 4096 entries and 64 MiB unpacked.
4. Reads `mod.json` and checks `id` and `version` against the index.
5. Copies the zip to the game folder's `mods/.disabled/<id>.zip`. The user enables it after seeing what it declares.
   An update replaces the old package and keeps it enabled if it was.

## Signatures

Planned, not checked yet. The index will carry `"signature": "<base64 Ed25519 signature>"` over the canonical bytes of
the index with the `signature` key removed (JSON with sorted keys, no spaces). The launcher will ship the public keys it
trusts and refuse an index with a bad signature; an unsigned index will show a warning. Because every archive is pinned
by its SHA-256 in the index, signing the index covers the archives too.

## HTTP API for a server (later)

A server that hosts mods (for example `castanea-backend`, see its `docs/protocol.md`) should offer, under `/v1/mods`:

| Request | Response |
|---|---|
| `GET /v1/mods/index.json` | The full index above. `ETag` and `Cache-Control: max-age=300`; `304` on `If-None-Match` |
| `GET /v1/mods?q=<text>&tag=<tag>&game=<version>&page=<n>` | `{"mods": [<mod without versions but with "latest": <version>>], "next": <n or null>}`, for a large index |
| `GET /v1/mods/{id}` | One mod with all its versions, as in the index |
| `GET /v1/mods/{id}/{version}/download` | `302` to the zip (a CDN or object store URL, `https://`) |
| `GET /v1/news` | `{"news": [...]}` as in the index |

Errors use the backend's shape `{"error": "<message>"}`. `404` for an unknown id or version, `429` with `Retry-After`
when limited. All responses are JSON over TLS. Uploads, accounts and moderation are out of scope for version 1; mods
are added to the index by its maintainers (a pull request to the index repository, for example).

The launcher needs only `index.json` today; the other endpoints are for when the index grows too large to load at once.
