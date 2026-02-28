# Evalify Proctor

A secure kiosk-mode proctoring application for the Evalify platform. Launches a Chromium-based browser in locked-down fullscreen, blocks unsafe shortcuts, disables DevTools, and routes all traffic through an authenticated local proxy.

## Supported platforms

| Platform | Desktop / Session | Status |
|----------|-------------------|--------|
| Windows  | Any               | Supported (requires Administrator) |
| Linux    | LXQt + X11        | Supported (requires root) |
| macOS    | —                 | Partial (browser launch only) |

## Prerequisites

- **Rust** stable toolchain (1.70+)
- **Chromium**, **Google Chrome**, or **Microsoft Edge**

### Linux-specific

- LXQt desktop with Openbox window manager
- X11 session (`XDG_SESSION_TYPE=x11`)
- `xmodmap` installed (usually part of `x11-xserver-utils`)

### Windows-specific

- Run as **Administrator** (needed to write registry policies)

## Quick start

```bash
# 1. Clone and enter the repo
git clone https://github.com/evalify/evalify-proctor.git
cd evalify-proctor

# 2. Create your .env from the example
cp env.example .env
# Then edit .env with your values

# 3. Build
cargo build --release

# 4. Generate the encrypted key blob (one-time)
./target/release/encrypt_key encrypted_blob.b64

# 5. Run
# Linux  — needs root for /etc policy writes
sudo -E ./target/release/evalify-kiosk

# Windows — run an elevated terminal, then:
.\target\release\evalify-kiosk.exe
```

## Configuration

All settings are read from environment variables (or a `.env` file in the project root).

| Variable | Description | Default |
|----------|-------------|---------|
| `TARGET_URL` | URL loaded in the kiosk browser | `http://evalify.amritanet.edu` |
| `PROXY_PORT` | Local proxy listen port | `8080` |
| `ALLOWED_DOMAINS` | Comma-separated domains the proxy will forward to | `evalify.amritanet.edu,localhost:3000` |
| `LOGOUT_PATHS` | Comma-separated URI paths that trigger auto-shutdown | `/api/auth/logout` |
| `ENCRYPT_PASSPHRASE` | Passphrase for AES-256-GCM key derivation (used by `encrypt_key`) | — |
| `KIOSK_KEY` | Raw kiosk identification key (encrypted by `encrypt_key`) | — |

## What it does

### 1. Proxy server

A local HTTP proxy starts on `127.0.0.1:<PROXY_PORT>`. The browser is configured to route all traffic through it. The proxy:

- **Blocks** requests to domains not in `ALLOWED_DOMAINS`.
- **Injects** an encrypted kiosk identity header (`X-Kioski-Encrypted`) into every forwarded request.
- **Detects logout** — when a request matches a `LOGOUT_PATHS` entry, the app shuts down gracefully.

### 2. Browser lockdown

The browser launches in kiosk/fullscreen mode with flags that disable extensions, incognito/private mode, DevTools, PDF viewer, print, translate, sync, and other escape routes.

### 3. DevTools policy

DevTools are disabled via OS-level browser policy so they cannot be re-enabled from within the browser.

| Platform | Mechanism |
|----------|-----------|
| Windows  | Registry keys under `HKLM\SOFTWARE\Policies\{Google\Chrome, Microsoft\Edge}` |
| Linux    | JSON policy files in `/etc/opt/chrome/`, `/etc/chromium/`, `/etc/opt/edge/` |

Policies are removed on teardown.

### 4. Input hooks

Dangerous keyboard shortcuts and right-click are intercepted at the OS level.

**Blocked shortcuts (both platforms):**

| Shortcut | Reason |
|----------|--------|
| Ctrl+T | New tab |
| Ctrl+W | Close tab |
| Ctrl+N | New window |
| Ctrl+Shift+T | Reopen closed tab |
| Ctrl+Shift+N | Incognito / private window |
| Ctrl+L | Focus address bar |
| Ctrl+Shift+I | DevTools |
| Ctrl+Shift+J | DevTools console |
| F11 | Toggle fullscreen |
| F12 | DevTools |
| Print Screen | Screenshot |
| Right-click | Context menu |

**Windows-only additional blocks:**

| Shortcut | Reason |
|----------|--------|
| Alt+Tab | Window switcher |
| Alt+Esc | Window cycle |
| Win+V | Clipboard history |

**Linux-only additional blocks:**

| Shortcut | Reason |
|----------|--------|
| Alt+Tab / Alt+Shift+Tab | Window switcher (Openbox) |
| Super key | LXQt launcher |
| Print key (X11 keycode) | Screenshot at X11 level |
| LXQt screenshot shortcuts | Disabled in `globalkeyshortcuts.conf` |

All hooks are uninstalled and original settings restored on shutdown.


### Key encryption utility

```bash
cargo run --bin encrypt_key -- encrypted_blob.b64
```

Reads `ENCRYPT_PASSPHRASE` and `KIOSK_KEY` from the environment, encrypts the key with AES-256-GCM (HKDF-derived key), and writes the base64 blob to the specified file. This blob is embedded into the binary at compile time via `include_str!`.

## Troubleshooting

**"Must run as root"** (Linux) / **Registry access denied** (Windows)
The app needs elevated privileges to write browser DevTools policies. Run with `sudo -E` on Linux or an Administrator terminal on Windows.

**"Only X11 sessions are supported"**
The Linux input hooks use `xmodmap`, which requires X11. Make sure your session type is X11 (`echo $XDG_SESSION_TYPE`). Wayland is not supported.

**"Browser not found"**
Install Chrome, Chromium, or Edge. On Linux the binary must be on `$PATH` (e.g. `google-chrome-stable`, `chromium-browser`, `microsoft-edge-stable`).

**Port conflict on startup**
Change `PROXY_PORT` in `.env` to an available port.

## License

Apache License 2.0 — see [LICENSE](LICENSE).
