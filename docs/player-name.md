# The player name

The launcher knows one player name. It is the name you type in Settings → Game → Player name, and until you type one it
is your computer account's name. Code: `crates/core/src/playername.rs` (rules), `crates/core/src/osname.rs` (the OS name).

## Which name is used

1. **The name you typed** (`launcher.json`, `game.player_name`). Once it is saved it wins for good, also when the account's
   name changes later.
2. **The OS account's display name**, while the field is empty. Clearing the field returns to it. It is read at start and is
   never written to `launcher.json`.
3. Nothing, when the OS gives no name at all: the launcher then passes no `--name` and the game keeps its own name.

Settings shows the OS name as the placeholder of the empty field. The home page says `Hello, <name>` (`Hallo, <name>`)
under the status line, with the name in use.

## Reading the OS name

| System | Where it comes from | Fallback |
|---|---|---|
| Windows | `GetUserNameExW(NameDisplay)` (secur32): the full name of the account, as the sign-in screen shows it | the `USERNAME` variable |
| Linux, macOS | the GECOS field of `getpwuid_r` for the real user (name service: files, LDAP, homed): its first comma part, `&` standing for the login name capitalised | the login name, then `USER` / `LOGNAME` |

The pieces that parse (`osname::gecos_name`, `passwd_fields`, `from_passwd`, `from_wide`) are pure functions with tests
for commas, an empty field, `&` and non-ASCII names.

## What the game gets

The game accepts 3 to 16 ASCII letters, digits or `_` (`network/link/ident.rs` of castanea). The launcher keeps and shows
your name as typed (white space collapsed, control and invisible characters removed, at most 32 characters) and passes
the closest allowed form as `--name` (`playername::for_game`):

| You have | The game gets |
|---|---|
| `Anna` | `Anna` |
| `Jürgen Müller` | `Juergen_Mueller` (umlauts and `ß` spelled out, accents dropped, a space becomes `_`) |
| `Alexander Hamilton-Smith` | `Alexander_Hamilt` (cut at 16) |
| `Al` | `Al_` (padded to 3) |
| `--world` | `world` (a name never reads as an option) |
| `日本語` | nothing: no `--name`, the game keeps its own name |

Settings shows what the game gets under the field (`In the game: Juergen_Mueller`).

## What the game does with it

At the time of writing the game reads `--name` only when it joins a server (`--join HOST:PORT --name NAME`); a start into
the start screen or a world ignores it, and the Multiplayer page of the start screen keeps its own saved name in
`multiplayer.txt` (default: the `USER` or `USERNAME` variable). So the launcher's name reaches the game in two places: a
Continue that joins a server, and a game that learns to use `--name` for its default (see "What the game would need" in
the quick start doc). The launcher does not write the game's files to put the
name there.

## Privacy

The name stays on this computer. The launcher sends no telemetry: its only requests are the release check (platform, channel,
product), the downloads, the news and the mod index, with the user agent `castanea-launcher/<version>` and nothing about you.
`crates/core/src/privacy_tests.rs` keeps the code that talks to the network from ever reading the name. The one place it
shows is the command line of the game in Game output and `logs/latest.log` on this computer.
