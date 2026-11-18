# Release API

The game repo is private, so the launcher does not read GitHub. It gets the game builds from the release server
(`https://castanea.xenoo.dev`, built in the `TerraNuxt/castanea-backend` repo). This page is the contract the launcher
is built against (`crates/core/src/install/release_api.rs`, tested against a local server that implements it in
`crates/core/src/release_server.rs`). `{base}` is the server address: the `release_url` setting (Settings → Advanced →
Release server), overridden by the environment variable `CASTANEA_RELEASE_URL`. Only `https://` is accepted, and
`http://` to `127.0.0.1`, `localhost` or `[::1]` for tests.

The server hosts two products, the **game** and the **launcher** itself. Every route takes `product=game|launcher`
(the game when it is left out, which is why the game's requests look as they always did). Every release is signed
([Signatures](#signatures)). The server side is documented in `castanea-backend` `docs/releases.md`; where the two
differ, that page wins.

## Platforms and channels

| | Values |
|---|---|
| `platform` | `linux-x86_64`, `windows-x86_64`. The launcher takes it from the OS and processor it runs on; on any other system it does not ask and the home page says there is no game build for this system yet. |
| `channel` | `stable` (the default), `beta` (only with Settings → Advanced → Include beta builds) |

With beta builds on, the launcher asks for `stable` and for `beta` and installs the one with the higher semver version, so
a newer stable release wins over an older beta. With them off it never sends `channel=beta`. The launcher does not need
`channel=beta` to include stable releases.

## Endpoints

### `GET {base}/v1/releases/latest?platform=P&channel=C[&product=launcher]`

`200`:

```json
{
  "product": "game",
  "version": "0.3.0",
  "channel": "stable",
  "platform": "linux-x86_64",
  "released_at": "2026-11-01T12:00:00Z",
  "size": 183746211,
  "sha256": "<64 lower-case hex digits>",
  "url": "{base}/v1/releases/0.3.0/linux-x86_64/download",
  "notes": "## What's new\n\n- Faster world loading",
  "signature": "<base64, 88 characters>",
  "key_id": "<16 lower-case hex digits>"
}
```

`signature` and `key_id` are `null` for a release published without one. The launcher refuses those (see below).
For the launcher product `url` is `{base}/v1/releases/<version>/<platform>/download?product=launcher`; the launcher
uses every `url` as given.

`notes` is markdown; the launcher shows it as plain text (headings and emphasis marks removed, list items with a bullet,
links as their text). `size` and `sha256` describe the zip. `url` must start with `{base}/`, or the release is refused
(the launcher never downloads from another host on the API's say). `version` is semver (a `v` prefix is tolerated).

`404` with `{"error":"no release"}`: nothing is published for this platform and channel yet. The launcher says "No game
release is published yet." and keeps the installed build. A `404` with any other body is treated as a wrong server
address, not as "no release".

### `GET {base}/v1/releases?platform=P&channel=C`

`200` `{"releases":[ <the same object as above>, … ]}`, newest first. The launcher client has it (`ReleaseApi::list`) but
the update check uses `latest` only: one install, always the newest release.

### `GET {base}/v1/releases/<version>/<platform>/download[?product=launcher]`

The zip, streamed. It carries `Content-Length`, `ETag: "<sha256>"` (the same hash as the release's `sha256`, in quotes)
and supports HTTP `Range` (`bytes=N-`, answered `206` with `Content-Range`), and honours `If-Range: "<sha256>"` (a
different ETag gets the whole file with `200`). When the server is rate limiting it answers `429` with `Retry-After:
<seconds>`.

The zip holds at its root the game executable (`castanea`, or `castanea.exe` on Windows) and the `assets/` folder, and
optionally `version.txt`. One top folder around everything is tolerated and dropped. A launcher zip holds
`castanea-launcher` (`castanea-launcher.exe`) at its root and optionally a `LICENSE`.

## Signatures

Ed25519 (RFC 8032) over exactly this UTF-8 text, lines joined with `\n`, no trailing newline:

```
castanea-release-v1
<product>
<version>
<platform>
<channel>
<sha256, lower-case hex of the zip>
<size of the zip, decimal bytes>
```

`signature` is the 64 raw bytes in standard padded base64. `key_id` is the first 8 bytes of the SHA-256 of the raw
32-byte public key, as 16 hex digits. The public keys are embedded in the launcher (`crates/core/src/trust.rs`,
`TRUST_KEYS`, several for rotation; a compile-time check refuses an empty list). The first one is the release key
published in `castanea-backend` `docs/releases.md` (`key_id` `c6dafdb0e8d6c8ff`).

What the launcher checks, in this order, for both products, before it unpacks a byte:

1. The finished download hashes (SHA-256) to the API's `sha256`, so the hash is the real file's.
2. The message is built from the API's `product` (the one asked for), `version`, `platform`, `channel` and **the
   downloaded file's own** hash and size, never from the API's text of them alone.
3. `signature` is present, valid base64 of 64 bytes, `key_id` names a trusted key, and the signature verifies
   (`verify_strict`). A tampered size, channel, version, platform or product fails here.

Any failure deletes the download, keeps the installed build or launcher, and shows "this download is not signed by
castanea" with the reason (no signature, malformed, unknown key, does not match). Test vectors (the key with seed
`00 01 … 1f`) are in `crates/core/src/signature/tests.rs` and in the backend's `docs/releases.md`; the tests also cover a
tampered size, a tampered channel, the wrong key, a truncated signature and a rotated key list.

Settings → Advanced → **Allow unsigned builds** (off by default) lets a *game* build with a missing or wrong signature
install. It is for developers and is ignored by release builds of the launcher (`Settings::unsigned_allowed`); the
launcher's own updates always need a signature. A local build folder is never downloaded, so it needs no signature.

`CASTANEA_TRUST_KEYS_FILE` (one base64 public key per line, `#` comments) replaces the embedded list in debug builds and
tests only; a release build ignores it, so a release cannot be redirected to other keys.

## News

The home page shows a News column (docs/ui-design.md, "Play (home)"). The launcher client is
`crates/core/src/news.rs` (tested in `crates/core/src/news/tests.rs` against a local server); the server side is built
by the backend against this contract and is **planned**: until it exists the launcher gets `404` and shows its quiet
"News could not be loaded" line, or the saved news.

### `GET {base}/v1/news?lang=en|de&limit=10`

`lang` is `en` or `de`, the launcher's language (any other value is sent as `en`); `limit` is how many items at most
(the launcher asks for 10, and keeps at most 20 whatever the server sends). Newest first. No `platform`, `channel` or
`product`: the news are the same for everyone, per language. The server should send an `ETag` and answer
`If-None-Match` with `304` (empty body); the launcher always sends it back when it has saved news.

`200`:

```json
{
  "items": [
    {
      "id": "steam-age",
      "title": "The steam age begins",
      "body": "Smelteries and the first steam machines are in. **Cast your first copper part**, then read\nhow it works in [the casting guide](https://castanea.xenoo.dev/docs/casting).\n\nA second paragraph.",
      "date": "2026-11-12",
      "tag": "release",
      "url": "https://castanea.xenoo.dev/news/steam-age"
    }
  ]
}
```

| Field | Rule |
|---|---|
| `id` | A stable string, unique in the list (up to 64 characters). Only used to remember which item is opened; a repeated id is dropped, a missing one becomes `#<position>`. |
| `title` | Plain text, one line, up to 120 characters. An item without a title is dropped. |
| `body` | Plain text, up to 1500 characters, `\n` for line breaks and a blank line for a paragraph. **No HTML.** Two forms are understood, nothing else: `**bold**` and `[text](https://…)`. Anything else, `<b>` and `&amp;` included, is shown as it is. May be empty. |
| `date` | `YYYY-MM-DD`, or a full ISO time (the launcher shows the day). Up to 32 characters. May be empty. |
| `tag` | A short label such as `release`, `launcher`, `community` (up to 24 characters), shown after the date. May be empty. Not translated by the launcher: the server sends it in `lang`. |
| `url` | Optional. `https://` only; "Read online" opens it in the system browser. |

Other answers: `304` (nothing changed), `404` or any other status (an error: the launcher keeps what it has saved), `429`
and `5xx` (the same; the news are never retried in a loop). The server writes the texts in the asked language; the
launcher does not translate or fall back between languages.

### What the launcher does with it

| Step | Rule |
|---|---|
| Untrusted | Everything from the server is data. The answer must be at most 256 KB (more is refused). Control characters and text-direction overrides are removed, every field is cut to its limit, at most 20 items are kept, `http://`, `javascript:`, `file:` and any other link is dropped (link text stays as text), a link or `url` with a user name before the host or with spaces is dropped. Bad JSON, a missing `items` list or a wrong type for a field never crashes: a bad item is skipped, a bad document is an error. |
| Request | One `GET` per launcher start and per language change, on a background thread; no retry; the whole request is dropped after 5 s. Not in offline mode (the saved news are shown). Never blocks Play, the update check or any other page. |
| Cache | `cache/news/<lang>.json` (`{"etag": …, "items": […]}`, written atomically, read through the same limits). Read when the launcher starts, so the news show at once and without a network. A `200` replaces it; a `304` keeps it; an error leaves it as it was. A damaged or oversized file reads as no news. |
| Offline | In offline mode, or when the request fails, the saved news stay on screen with a small "offline · saved" mark; with nothing saved a one-line message says so. |
| Links | A click on "Read online" or a link in the text opens the address in the system browser (`xdg-open`, `explorer`). The launcher shows the address as a tooltip. |
| Language | The news follow the launcher's language setting; changing it shows the saved news of the other language and asks again. |

## What the launcher does with it

| Step | Rule |
|---|---|
| Check | Once at start. Not in offline mode, not with a local build folder. Failure never blocks playing the installed build. |
| Install when | the version differs from the installed one and is not older. The same version is a no-op; there is no downgrade. |
| Partial file | `cache/downloads/<sha256>.part`. It stays after a network drop, a 429/5xx, a cancel and a restart. |
| Resume | `Range: bytes=<size of the partial file>-` with `If-Range: "<sha256>"`. A `206` must start at that byte and, if `Content-Range` gives a total, the total must be `size`; otherwise the file starts again. A `200` replaces the partial file from byte 0. |
| ETag | If the response has an `ETag`, it must equal `"<sha256>"` of the release (weak `W/` prefix tolerated). Otherwise the launcher refuses ("not the release's build"), deletes the partial file and keeps the installed build. A response without an ETag is accepted, the SHA-256 below decides. |
| Verify | The finished file is hashed (SHA-256) and compared with the API's `sha256`, and its length with `size`. A mismatch deletes the file; the installed build stays; the home page says the download failed. |
| Signature | After the hash: see [Signatures](#signatures). Nothing is unpacked before it holds. |
| Unpack | The existing zip limits (entry count, unpacked size, no `..`, no absolute paths, no symlinks). The result must have the platform's executable and `assets/`, else it is refused and deleted. |
| Swap | `build/next` → `build/current` through `build/previous`; `previous` is deleted after the swap, put back on failure, and only `current` is left (docs/design.md). |
| Notes | Kept in `install.json`; "What's new" on the home page shows them (during the download: those of the new version). |

## Error handling

| Situation | Launcher |
|---|---|
| Network down, DNS or TLS error while checking | The status line says it could not check; the error is the tooltip; Try again; the installed build runs. |
| `404 {"error":"no release"}` | "No game release is published yet." (only a problem when nothing is installed: Play stays off.) |
| `404` with another body | Error "the release server has no such address (is it the right server?)". |
| `429`, `5xx` on `latest` | Retried up to 5 times, waiting `Retry-After` seconds (else 1, 2, 4, 8, 16 s; never more than 60 s). Then an error (`429`: "the release server is busy, try again in a minute"). |
| `429`, `5xx` on the download | Retried the same way, keeping the partial file. The count of failures in a row starts again whenever bytes arrived. |
| Connection drops in the middle of the body, or ends before `Content-Length` | Counted as a failure; waits and resumes with `Range`. |
| `416` | The partial file is deleted and the download starts at byte 0 (at most 3 times). |
| Other `4xx` | Error with the status; nothing is deleted except an unusable partial file. |
| ETag of another build | Refused, partial file deleted (see above). |
| SHA-256 or size mismatch | File deleted, installed build kept, error shown. |
| Signature missing, malformed, unknown key or invalid | File deleted, nothing unpacked, installed build or launcher kept, "this download is not signed by castanea" shown. |
| Archive without executable or `assets/`, unsafe paths, over the limits | Refused, `build/next` deleted, installed build kept. |
| Platform not supported | No request; "There is no game build for this system yet." |
| `url` not on `{base}`, wrong `platform`, bad `sha256`, `size` 0 | The release is refused as invalid. |
| Cancel | The partial file stays for the next try. |

## Proposed changes (not built against; the launcher works without them)

- `channel=beta` could mean "newest of beta and stable" on the server, so the launcher needs one request. Today it asks
  for both and compares.
- `ETag` and `If-None-Match` on `latest` (a cheap `304` for every start).
- The server side of [News](#news) (`GET /v1/news`), which the launcher already asks for.
- `min_launcher_version` in a release, so the server can tell an old launcher that the zip layout changed.
- `Content-Type: application/zip` and `Content-Disposition` on the download (the launcher does not rely on them).
- `Retry-After` as an HTTP date is not parsed (only seconds); the server should send seconds.
