# Repository Research Report

## Executive Summary

This repository contains a small Rust Telegram bot that downloads videos from Twitter/X links and sends the resulting video back into the Telegram chat. The runtime is built around `teloxide`, `tokio`, and an external `yt-dlp` process. The intended production environment is NixOS, using a flake-built package, a NixOS systemd module, and SOPS-managed secrets for the Telegram bot token and X cookie file.

The current system has three main operating surfaces:

1. The Rust bot process, which validates environment configuration, listens for Telegram messages, enforces a Telegram sender allowlist, invokes `yt-dlp`, and replies with videos or errors.
2. The NixOS module, which creates the `xbot` service user, wires SOPS secrets into an environment file, runs the package as a hardened systemd service, and exposes `yt-dlp`/`ffmpeg` in the service `PATH`.
3. The archive script, which moves completed `video.mp4` files out of per-task working directories into `/data/xbot/videos` and cleans up stale task directories that only contain the copied cookie file.

The codebase is compact and easy to reason about, but it is operationally specific. It assumes Linux, `yt-dlp`, readable cookie secrets, writable bot state directories, and a NixOS/SOPS deployment workflow.

## Repository Layout

Top-level files:

- `Cargo.toml` and `Cargo.lock` define the Rust crate `telegram_x_video_bot`.
- `flake.nix` and `flake.lock` define the Nix package, overlay, and NixOS module export.
- `nix/module.nix` defines the production service configuration.
- `readme.md` documents SOPS basics, sender allowlist configuration, and video archive setup.
- `.sops.yaml` declares age recipients for files matching `secrets/*.yaml`.
- `.gitignore` excludes local build output, videos, local env files, raw cookie files, Nix result symlinks, and the local `secrets/` directory.
- `.dockerignore` exists for the older Docker build path and excludes env/secrets/build/docs-related files.

Rust source files:

- `src/main.rs`: runtime entry point.
- `src/config.rs`: environment loading and validation.
- `src/bot.rs`: Telegram message handling and allowlist enforcement.
- `src/storage.rs`: per-task output directory creation.
- `src/ytdlp.rs`: `yt-dlp` invocation and output video detection.

Operational files:

- `scripts/xbot-video-archive.sh`: moves completed downloads to a long-term archive path.
- `docs/sops-nixos-troubleshooting.md`: documents SOPS/age troubleshooting for NixOS deployments.
- `.github/workflows/docker-build-push.yml` and `.github/workflows/prod.yml.bak`: older Docker image workflows. These do not appear to be the primary deployment path described by current repo guidance.

Host-side files reviewed outside this repository:

- `/etc/nixos/flake.nix`: the local NixOS system flake that consumes this repository.
- `/etc/nixos/xbot-archive.nix`: the local NixOS timer/service for the archive script.

## What The Bot Does

The bot listens to Telegram messages and, for allowed users only, looks for text containing `twitter.com` or `x.com`. If a text message contains one of those substrings, it treats the full message text as the URL/input for `yt-dlp`, downloads the best MP4-compatible video it can get, and sends that video back to the same Telegram chat.

For messages from allowed users that do not contain `twitter.com` or `x.com`, it replies with:

```text
Please provide a valid Twitter/X video link.
```

For messages from unauthorized users, it does not reply. It logs a debug message with the chat ID and optional sender user ID, then returns.

The bot is intentionally not a general downloader. Its user-facing trigger is Twitter/X-specific, even though the underlying `yt-dlp` command may technically support more sites.

## Runtime Flow

`src/main.rs` performs the process-level startup:

1. Initializes `pretty_env_logger`.
2. Logs `Starting X video downloader bot...`.
3. Loads `AppConfig` from environment.
4. Calls `bot::run(cfg).await`.

`src/config.rs` loads and validates:

- `TELOXIDE_TOKEN`: required. Wrapped in `secrecy::SecretString`.
- `COOKIE_FILE`: required. Must exist at startup.
- `VIDEO_DIR`: optional. Defaults to `video`.
- `ALLOWED_USER_IDS`: required. Parsed as comma-separated numeric Telegram user IDs.

The allowlist parser trims whitespace around entries, rejects empty entries like `123,,456`, rejects non-numeric entries, deduplicates IDs through a `HashSet<UserId>`, and requires at least one configured user.

`src/bot.rs` then:

1. Constructs `Bot::new(cfg.token_str())`.
2. Tries `get_me()` and logs the connected bot username if successful.
3. Starts a `teloxide::repl`.
4. For each message:
   - Extracts `msg.from.as_ref().map(|user| user.id)`.
   - Checks that sender ID against `cfg.allowed_user_ids`.
   - Ignores unauthorized or sender-less messages.
   - For authorized text messages containing `twitter.com` or `x.com`, sends a temporary `Downloading video...` message.
   - Creates a task directory under `VIDEO_DIR`.
   - Calls `ytdlp::download(text, &cfg.cookie_file, &task_dir)`.
   - Sends the downloaded file with `send_video`.
   - Deletes the temporary processing message after successful send.
   - Sends a failure reply when directory creation or download fails.

Important behavior detail: the processing message is deleted only on successful video send or when task directory creation fails. If `yt-dlp` fails after the processing message has been sent, the code sends a failure message but leaves the original processing message in the chat.

## Download Implementation

`src/ytdlp.rs` is the boundary between Rust and the external downloader.

`download(url, cookie_file, out_dir)` does the following:

1. Creates `out_dir` with `tokio::fs::create_dir_all`.
2. Copies the configured cookie file into the task directory as `cookie.txt`.
3. On Unix, sets copied cookie permissions to `0600`.
4. Builds output template `out_dir/video.%(ext)s`.
5. Runs:

```bash
yt-dlp -f best[ext=mp4]/best --cookies <task_dir>/cookie.txt -o <task_dir>/video.%(ext)s <url>
```

6. If `yt-dlp` exits unsuccessfully, returns an error containing trimmed stderr, capped at 1200 bytes.
7. Reads the output directory and returns the first regular file whose name:
   - starts with `video.`
   - does not end with `.part`
   - is not `cookie.txt`

The cookie copy is deliberate. The NixOS SOPS secret file is read-only, and `yt-dlp` may try to update or save cookies. Copying the cookie into a writable task directory prevents writes against the read-only secret path.

The output matching accepts any `video.<ext>` file, not only MP4. The format selector asks for `best[ext=mp4]/best`, so MP4 is preferred, but fallback output could have another extension if `yt-dlp` cannot get MP4.

## Storage Model

`src/storage.rs` creates a per-task directory:

```text
<VIDEO_DIR>/<YYYY-MM-DD_HH-MM-SS>_<chat_id>/
```

The timestamp uses local system time through `chrono::Local`. The chat ID is included in the directory name and may be negative for Telegram supergroups/channels.

The function returns `(PathBuf, String)`, where the string is the timestamp. Current caller code ignores the timestamp and only uses the path.

Specificity to note: directory names have one-second timestamp precision. Two authorized requests in the same chat within the same second will resolve to the same task directory. In that case, the two downloads would share `cookie.txt` and the `video.%(ext)s` output template. There is no explicit collision avoidance beyond chat ID and timestamp.

## Telegram Authorization Model

The sender allowlist is enforced before link validation and before any user-facing reply. This prevents unauthorized users from probing bot behavior through responses.

The allowlist is based on Telegram user IDs, not chat IDs. This means:

- A whitelisted user can use the bot in any chat where the bot receives their message.
- A non-whitelisted user in an otherwise valid chat is ignored.
- Messages without a sender user are ignored.

The NixOS module treats allowed user IDs as non-secret plain configuration:

```nix
services.xbot.allowedUserIds = [ 123456789 987654321 ];
```

The module converts that list to a comma-separated `ALLOWED_USER_IDS` value in the generated systemd environment file.

## Configuration And Secrets

Required runtime variables:

- `TELOXIDE_TOKEN`: Telegram bot token.
- `COOKIE_FILE`: path to the X/Twitter cookie file.
- `ALLOWED_USER_IDS`: comma-separated numeric Telegram user IDs.

Optional runtime variable:

- `VIDEO_DIR`: base output directory. Defaults to `video` for local development.

The NixOS module generates these values through SOPS templates:

- `TELOXIDE_TOKEN` comes from SOPS key `teloxide_token`.
- `COOKIE_FILE` points to the SOPS-managed secret file for key `x_cookie_txt`.
- `VIDEO_DIR` is fixed by the module to `/var/lib/xbot/videos`.
- `ALLOWED_USER_IDS` is derived from `services.xbot.allowedUserIds`.

The generated template also includes:

```text
MAX_CONCURRENT=2
TIMEOUT_SECS=600
```

Current Rust code does not read or enforce either of those variables. They are operationally inert at the application level unless future code starts using them.

## Nix Flake And Package

`flake.nix` supports:

- `x86_64-linux`
- `aarch64-linux`

The package is built with `pkgs.rustPlatform.buildRustPackage`:

- `pname = "xbot"`
- `version = "0.1.0"`
- `src = ./.`
- `cargoLock.lockFile = ./Cargo.lock`
- `nativeBuildInputs = [ pkgs.pkg-config ]`
- `buildInputs = [ pkgs.openssl ]`

The OpenSSL inputs are necessary because the dependency graph includes `openssl-sys`, likely through Telegram/networking dependencies.

The flake exports:

- `packages.<system>.xbot`
- `packages.<system>.default`
- `overlays.default`
- `nixosModules.default`

The service module references the package as `${pkgs.xbot}/bin/telegram_x_video_bot`, so host configuration must make the overlay/package available as `pkgs.xbot` when using the module in that form.

## Host NixOS Integration

The host flake at `/etc/nixos/flake.nix` shows how this repository is integrated on the actual machine.

It declares:

```nix
xbot.url = "path:/home/harvey/github-codes/telegram_x_video_bot";
```

This means the NixOS system consumes the local checkout directly rather than a remote Git URL. Changes in this repository can therefore become part of the system rebuild once the host flake lock/input state is refreshed as needed.

The host flake defines one configuration:

```nix
nixosConfigurations.hpworkstation
```

for:

```nix
system = "x86_64-linux";
```

Its module list includes:

- `/etc/nixos/hardware-configuration.nix`
- `/etc/nixos/configuration.nix`
- an inline overlay module
- `xbot.nixosModules.default`

The inline overlay is important:

```nix
nixpkgs.overlays = [ xbot.overlays.default ];
```

This satisfies the repo module's assumption that the package is available as `pkgs.xbot`. Without that overlay, `nix/module.nix` would not be able to resolve:

```nix
${pkgs.xbot}/bin/telegram_x_video_bot
```

The host also passes `sops-nix` as an input, while this repository's flake separately wires `sops-nix` into its exported NixOS module. The result is a flake-based deployment path where the bot package, overlay, and service module all come from this checkout.

## NixOS Service Module

`nix/module.nix` imports `sops-nix.nixosModules.sops` and defines `services.xbot.allowedUserIds`.

The module creates:

- System user `xbot`.
- System group `xbot`.
- SOPS secrets owned by `xbot:xbot` with mode `0400`.
- SOPS-generated environment file `xbot.env`, also owned by `xbot:xbot` with mode `0400`.
- `/var/lib/xbot/videos` via tmpfiles with mode `0700`.
- systemd service `xbot`.

The service:

- Runs as `User=xbot`, `Group=xbot`.
- Starts after and wants `network-online.target`.
- Adds `yt-dlp` and `ffmpeg` to `PATH`.
- Uses `EnvironmentFile` from the SOPS template.
- Runs `${pkgs.xbot}/bin/telegram_x_video_bot`.
- Restarts on failure with a 2 second delay.
- Sets `StateDirectory=xbot` and `CacheDirectory=xbot`.
- Sets `HOME=/var/lib/xbot`.
- Sets `XDG_CACHE_HOME=/var/lib/xbot/cache`.
- Uses `WorkingDirectory=/var/lib/xbot`.
- Uses `UMask=0077`.
- Enables hardening settings:
  - `NoNewPrivileges=true`
  - `PrivateTmp=true`
  - `ProtectSystem=strict`
  - `ProtectHome=true`
- Allows writes to `/var/lib/xbot` and `/var/lib/xbot/videos`.

The writable `HOME` and cache path matter because `yt-dlp` and supporting tools can expect cache/config paths. The hardened service would otherwise block many filesystem writes.

The module includes an assertion that `services.xbot.allowedUserIds` cannot be empty. This aligns with the Rust startup validation for `ALLOWED_USER_IDS`.

## Archive Script

`scripts/xbot-video-archive.sh` is a separate operational cleanup/migration tool. Its default paths are:

- Source: `/var/lib/xbot/videos`
- Destination: `/data/xbot/videos`

These can be overridden with `SRC_DIR` and `DST_DIR`.

It scans one directory level under the source directory and expects task directories matching:

```text
YYYY-MM-DD_HH-MM-SS_<chat_id>
```

where `<chat_id>` may be negative.

For each matching task directory, it looks specifically for:

```text
video.mp4
```

If present, it moves it to:

```text
<DST_DIR>/video_YYYY-MM-DD_HH-MM-SS.mp4
```

If the destination name already exists, it appends `_1`, `_2`, and so on.

After a move, or when a task directory is missing `video.mp4`, it attempts to clean up only directories that contain no files other than `cookie.txt`. It removes `cookie.txt` first, then removes the empty directory.

Important mismatch: the Rust downloader can return any `video.<ext>` file if `yt-dlp` falls back from MP4, but the archive script only moves `video.mp4`. Non-MP4 outputs would be skipped and left in task directories.

The host-side `/etc/nixos/xbot-archive.nix` implements the timer described in `readme.md`. It defines a oneshot service named `xbot-video-archive` that:

- Runs as `User=xbot`, `Group=xbot`.
- Adds `bash`, `coreutils`, and `findutils` to `PATH`.
- Executes `/var/lib/xbot/bin/xbot-video-archive.sh`.
- Uses `/` as the working directory.
- Sets `SRC_DIR=/var/lib/xbot/videos`.
- Sets `DST_DIR=/data/xbot/videos`.

It also defines `systemd.timers.xbot-video-archive` with:

```nix
OnCalendar = "*:0/5";
Persistent = true;
Unit = "xbot-video-archive.service";
```

So on the reviewed host, archiving is expected to run every five minutes and catch up after missed timer events. The timer file assumes the script has already been installed to `/var/lib/xbot/bin/xbot-video-archive.sh` with permissions suitable for the `xbot` user, as documented in the README.

## Documentation

`readme.md` currently documents:

- Basic SOPS recipient helper commands.
- Link to SOPS/NixOS troubleshooting.
- Sender allowlist configuration.
- Video archive behavior.
- Permission setup for `/data/xbot/videos`.
- Installing the archive script to `/var/lib/xbot/bin`.
- A recommended NixOS systemd timer that runs the archive every five minutes.
- Manual archive testing and log commands.

`docs/sops-nixos-troubleshooting.md` documents:

- Why SOPS may fail to decrypt on NixOS.
- How to point SOPS at an age key file.
- How to use an SSH private key as an age identity.
- How to verify recipients.
- How to manage `.sops.yaml` recipients safely.
- How to re-encrypt secret files.
- The important `teloxide_token` spelling expected by `nix/module.nix`.
- Security notes about plaintext raw secret files and token/cookie rotation.

## Tests

Current tests are unit tests embedded in `config.rs` and `bot.rs`.

Covered behavior:

- `ALLOWED_USER_IDS` parsing with whitespace.
- Rejection of empty allowlist entries.
- Rejection of non-numeric allowlist entries.
- `AppConfig::from_env` fails when `ALLOWED_USER_IDS` is missing.
- Allowlist helper permits whitelisted users.
- Allowlist helper denies non-whitelisted users.
- Allowlist helper denies missing sender users.

Not currently covered:

- `storage::make_task_dir` path format and collision behavior.
- `ytdlp::download` success/failure paths.
- Handling when `yt-dlp` is missing.
- Output file selection when multiple `video.*` files exist.
- Telegram handler behavior around processing-message deletion on failure.
- Archive script behavior.
- Nix module evaluation.

## Build And Verification Notes

I attempted both:

```bash
cargo test
cargo check
```

Both failed before compiling this crate's own code because `openssl-sys` could not find an OpenSSL installation through `pkg-config` in the current shell:

```text
Could not find openssl via pkg-config
The file `openssl.pc` needs to be installed and the PKG_CONFIG_PATH environment variable must contain its parent directory.
```

This is consistent with the flake package explicitly providing `pkg-config` and `openssl`. On NixOS, validation should be run through an environment that supplies those build inputs, for example a suitable nix shell/dev shell or `nix build .#xbot`.

## Notable Findings

### Processing Message Cleanup Is Asymmetric

On successful download and send, the temporary `Downloading video...` message is deleted. On task directory creation failure, the code also attempts to delete it. On `yt-dlp` failure, the code sends a failure message but does not delete the temporary processing message.

This is not fatal, but users may see stale progress messages after failed downloads.

### Config Includes Unused Concurrency And Timeout Values

The NixOS environment template sets `MAX_CONCURRENT=2` and `TIMEOUT_SECS=600`, but Rust code never reads these variables. The bot currently does not enforce a concurrency limit or a download timeout.

Because `teloxide::repl` can process updates concurrently depending on dispatcher behavior, downloads may overlap. Each overlapping download spawns a `yt-dlp` process.

### Task Directory Names Can Collide

Task directories are based on local timestamp to the second plus chat ID. Two requests from the same chat within the same second produce the same directory and output template.

Possible effects:

- Shared temporary `cookie.txt`.
- Competing writes to `video.%(ext)s`.
- Ambiguous output detection.
- Archive collision handling only after files reach `/data/xbot/videos`, not within the task directory.

Adding a message ID, update ID, random suffix, or nanosecond timestamp would make task directories safer.

### URL Detection Is Simple Substring Matching

The bot triggers on any message text containing `twitter.com` or `x.com`. It does not parse URLs, validate hostnames, or extract a single URL from the message.

Consequences:

- A message with explanatory text plus a URL is passed whole to `yt-dlp`.
- A domain containing those substrings could trigger unexpectedly.
- Multiple links in one message are passed as one argument to `yt-dlp`.

This may be acceptable for personal bot use, but it is intentionally simple rather than robust URL parsing.

### Archive Script Assumes MP4

The downloader prefers MP4 but can fall back to `best`, which may produce another extension. The archive script only moves `video.mp4`. If `yt-dlp` returns `video.webm` or another extension, the Telegram send path can still work, but the archive script will skip that task directory.

### The Cookie Copy Persists Until Archived Or Cleaned

Every download task directory receives a copied `cookie.txt` with mode `0600`. The archive script removes stale directories that contain only `cookie.txt`, and removes `cookie.txt` after moving `video.mp4`. Until that cleanup runs, copied cookie files remain under `/var/lib/xbot/videos`.

The NixOS service uses restrictive ownership, `UMask=0077`, and `/var/lib/xbot/videos` mode `0700`, which mitigates this on the host.

### Docker Workflow Appears Stale Relative To Current Deployment

There is a GitHub workflow for Docker image publishing and a backup workflow for Docker Hub, but the repository does not currently show a `Dockerfile` in the file list. `.dockerignore` also excludes `Dockerfile`, which is unusual for active Docker builds.

Current project-specific instructions and docs emphasize NixOS flakes, SOPS, and the `xbot` system user, so Docker appears historical or incomplete.

## Operational Model

Expected production model:

1. Host flake imports this repository from `path:/home/harvey/github-codes/telegram_x_video_bot`.
2. Host flake applies `xbot.overlays.default` so `pkgs.xbot` is available.
3. Host imports `xbot.nixosModules.default`.
4. Host config sets `services.xbot.allowedUserIds`.
5. SOPS provides:
   - Telegram bot token under key `teloxide_token`.
   - X cookie text under key `x_cookie_txt`.
6. NixOS creates the `xbot` user and locked-down service.
7. systemd starts the bot from the Nix-built package.
8. Bot writes task directories under `/var/lib/xbot/videos`.
9. `/etc/nixos/xbot-archive.nix` defines a five-minute archive timer that moves completed `video.mp4` files to `/data/xbot/videos`.

Expected local development model:

1. Provide `TELOXIDE_TOKEN`, `COOKIE_FILE`, and `ALLOWED_USER_IDS` in the environment.
2. Optionally set `VIDEO_DIR`; otherwise local `video/` is used.
3. Ensure `yt-dlp` is available in `PATH`.
4. Ensure OpenSSL development inputs are available for Rust builds.
5. Run `cargo run`, `cargo test`, or `cargo check`.

On NixOS, plain `cargo` from an arbitrary shell may fail if OpenSSL/pkg-config paths are not available. The flake package handles this for Nix builds.

## Dependency Notes

Direct Rust dependencies:

- `teloxide` with `macros`: Telegram bot framework.
- `tokio` with multi-thread runtime, macros, process, time, and fs features.
- `chrono`: local timestamp formatting for task directories.
- `anyhow`: error propagation with context.
- `secrecy`: protects token display/debug exposure.
- `log` and `pretty_env_logger`: logging.

External runtime dependencies:

- `yt-dlp`: required for downloads.
- `ffmpeg`: provided in the service path and commonly needed by `yt-dlp` for muxing/conversion.
- Telegram Bot API network access.
- Valid X/Twitter cookies.

Nix build dependencies:

- `pkg-config`
- `openssl`

## Suggested Future Improvements

High value:

- Add a unique suffix to task directories to prevent same-second collisions.
- Delete the processing message when `yt-dlp` fails.
- Either implement `TIMEOUT_SECS` and `MAX_CONCURRENT`, or remove them from the generated environment until used.
- Align archive behavior with downloader output by archiving `video.*`, not only `video.mp4`, or make the downloader guarantee MP4.
- Add tests for `storage::make_task_dir` and pure helper logic around output file selection.

Medium value:

- Parse URLs instead of using raw substring detection.
- Extract the first valid Twitter/X URL and pass only that URL to `yt-dlp`.
- Add integration-style tests around a fake `yt-dlp` executable.
- Add a Nix module evaluation test or at least document a `nix flake check` path.
- Consider making `VIDEO_DIR` configurable through the NixOS module instead of fixed to `/var/lib/xbot/videos`.

Lower priority:

- Remove or refresh stale Docker workflow files if Docker deployment is no longer supported.
- Expand `readme.md` with a concise end-to-end local development section.
- Document how to discover a Telegram numeric user ID for allowlisting.

## Overall Assessment

The repository is purpose-built and cohesive: it solves one workflow, with a clear split between bot logic, download execution, storage, NixOS service configuration, and archival maintenance. Its strongest parts are the early config validation, sender allowlist, SOPS-aware cookie handling, and hardened NixOS service setup.

The main risks are around operational edge cases rather than architectural complexity: concurrent downloads can collide, configured timeout/concurrency variables are not enforced, archive behavior assumes MP4 while the downloader can fall back to other formats, and failed downloads leave progress messages behind. These are all tractable improvements within the existing structure.
