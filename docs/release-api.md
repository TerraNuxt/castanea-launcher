# Release API

The game repo is private, so the launcher does not read GitHub. It gets the game builds from the release server
(`https://castanea.xenoo.dev`, built in the `TerraNuxt/castanea-backend` repo). This page is the contract the launcher
is built against (`crates/core/src/install/release_api.rs`, tested against a local server that implements it in
`crates/core/src/release_server.rs`). `{base}` is the server address: the `release_url` setting (Settings → Advanced →
Release server), overridden by the environment variable `CASTANEA_RELEASE_URL`. Only `https://` is accepted, and
`http://` to `127.0.0.1`, `localhost` or `[::1]` for tests.

## Platforms and channels

| | Values |
|---|---|
| `platform` | `linux-x86_64`, `windows-x86_64`. The launcher takes it from the OS and processor it runs on; on any other system it does not ask and the home page says there is no game build for this system yet. |
| `channel` | `stable` (the default), `beta` (only with Settings → Advanced → Include beta builds) |

With beta builds on, the launcher asks for `stable` and for `beta` and installs the one with the higher semver version, so
a newer stable release wins over an older beta. With them off it never sends `channel=beta`. The launcher does not need
`channel=beta` to include stable releases.

## Endpoints

### `GET {base}/v1/releases/latest?platform=P&channel=C`

`200`:

```json
{
  "version": "0.3.0",
  "channel": "stable",
  "platform": "linux-x86_64",
  "released_at": "2026-11-01T12:00:00Z",
  "size": 183746211,
  "sha256": "<64 lower-case hex digits>",
  "url": "{base}/v1/releases/0.3.0/linux-x86_64/download",
  "notes": "## What's new\n\n- Faster world loading"
}
```

`notes` is markdown; the launcher shows it as plain text (headings and emphasis marks removed, list items with a bullet,
links as their text). `size` and `sha256` describe the zip. `url` must start with `{base}/`, or the release is refused
(the launcher never downloads from another host on the API's say). `version` is semver (a `v` prefix is tolerated).

`404` with `{"error":"no release"}`: nothing is published for this platform and channel yet. The launcher says "No game
release is published yet." and keeps the installed build. A `404` with any other body is treated as a wrong server
address, not as "no release".

### `GET {base}/v1/releases?platform=P&channel=C`

`200` `{"releases":[ <the same object as above>, … ]}`, newest first. The launcher client has it (`ReleaseApi::list`) but
the update check uses `latest` only: one install, always the newest release.

### `GET {base}/v1/releases/<version>/<platform>/download`

The zip, streamed. It carries `Content-Length`, `ETag: "<sha256>"` (the same hash as the release's `sha256`, in quotes)
and supports HTTP `Range` (`bytes=N-`, answered `206` with `Content-Range`), and honours `If-Range: "<sha256>"` (a
different ETag gets the whole file with `200`). When the server is rate limiting it answers `429` with `Retry-After:
<seconds>`.

The zip holds at its root the game executable (`castanea`, or `castanea.exe` on Windows) and the `assets/` folder, and
optionally `version.txt`. One top folder around everything is tolerated and dropped.

## What the launcher does with it

| Step | Rule |
|---|---|
| Check | Once at start. Not in offline mode, not with a local build folder. Failure never blocks playing the installed build. |
| Install when | the version differs from the installed one and is not older. The same version is a no-op; there is no downgrade. |
| Partial file | `cache/downloads/<sha256>.part`. It stays after a network drop, a 429/5xx, a cancel and a restart. |
| Resume | `Range: bytes=<size of the partial file>-` with `If-Range: "<sha256>"`. A `206` must start at that byte and, if `Content-Range` gives a total, the total must be `size`; otherwise the file starts again. A `200` replaces the partial file from byte 0. |
| ETag | If the response has an `ETag`, it must equal `"<sha256>"` of the release (weak `W/` prefix tolerated). Otherwise the launcher refuses ("not the release's build"), deletes the partial file and keeps the installed build. A response without an ETag is accepted, the SHA-256 below decides. |
| Verify | The finished file is hashed (SHA-256) and compared with the API's `sha256`, and its length with `size`. A mismatch deletes the file; the installed build stays; the home page says the download failed. |
| Unpack | The existing zip limits (entry count, unpacked size, no `..`, no absolute paths, no symlinks). The result must have the platform's executable and `assets/`, else it is refused and deleted. |
| Swap | `build/next` → `build/current` through `build/previous`, put back on failure (docs/design.md). |
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
| Archive without executable or `assets/`, unsafe paths, over the limits | Refused, `build/next` deleted, installed build kept. |
| Platform not supported | No request; "There is no game build for this system yet." |
| `url` not on `{base}`, wrong `platform`, bad `sha256`, `size` 0 | The release is refused as invalid. |
| Cancel | The partial file stays for the next try. |

## Proposed changes (not built against; the launcher works without them)

- `channel=beta` could mean "newest of beta and stable" on the server, so the launcher needs one request. Today it asks
  for both and compares.
- `ETag` and `If-None-Match` on `latest` (a cheap `304` for every start).
- `min_launcher_version` in a release, so the server can tell an old launcher that the zip layout changed.
- A detached signature (`signature`, Ed25519 over the `sha256`) so the launcher can verify more than the TLS connection.
- `Content-Type: application/zip` and `Content-Disposition` on the download (the launcher does not rely on them).
- `Retry-After` as an HTTP date is not parsed (only seconds); the server should send seconds.
