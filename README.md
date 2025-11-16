# Evalify Kiosk

A secure kiosk application that launches Chromium in kiosk mode and provides an authenticated proxy for the Evalify platform.

## Overview

This application consists of two main components:
1. **Main Kiosk Application** (`evalify-kiosk`) - Launches Chromium in kiosk mode and runs an authenticated proxy server
2. **Key Encryption Utility** (`encrypt_key`) - Encrypts sensitive keys using AES-256-GCM encryption

## Prerequisites

- Rust (latest stable version)
- Chromium or Google Chrome browser
- Linux environment (tested on Linux systems)

## Setup

### 1. Environment Configuration

Copy the example environment file and configure your settings:

```bash
cp env.example .env
```

Edit the `.env` file with your configuration:

```env
ENCRYPT_PASSPHRASE="your-encryption-passphrase-here"
BACKEND_BASE_URL="http://evalify.amritanet.edu"
EVALIFY_URL="http://evalify.amritanet.edu"
LOCAL_AUTH_KEY="your-local-auth-key-here"
KIOSK_KEY="your-kiosk-key-here"
```

### 2. Build Release Binaries

Build optimized release binaries:

```bash
cargo build --release
```

This will create the following binaries in `target/release/`:
- `evalify-kiosk` - Main kiosk application
- `encrypt_key` - Key encryption utility

## Running the Application

### Step 1: Generate Encrypted Key (Optional)

If you need to generate a new encrypted key blob:

```bash
./target/release/encrypt_key encrypted_blob.b64
```

This will create/update the `encrypted_blob.b64` file with your encrypted kiosk key.

### Step 2: Run the Kiosk Application

#### Option A: Run directly from release binary

```bash
./target/release/evalify-kiosk
```

#### Option B: Run with custom environment variables

```bash
EVALIFY_URL="https://your-custom-url.com" \
BACKEND_BASE_URL="https://your-backend.com" \
LOCAL_AUTH_KEY="your-auth-key" \
./target/release/evalify-kiosk
```

### Step 3: Access the Application

Once running, the application will:

1. **Launch Chromium** in kiosk mode pointing to the configured Evalify URL
2. **Start a proxy server** on `http://127.0.0.1:8473`

The Chromium browser will open automatically in fullscreen kiosk mode.

## Application Features

### Kiosk Mode Configuration

The application launches Chromium with extensive security and kiosk-specific flags:
- Full-screen kiosk mode
- Disabled extensions, plugins, and external access
- Incognito mode for session isolation
- Disabled user interactions (printing, saving, etc.)
- Enhanced security settings

### Authenticated Proxy

- **Local Authentication**: Requires `X-Local-Auth` header matching your configured key
- **Origin Validation**: Only accepts requests from the configured Evalify URL
- **Encrypted Headers**: Adds encrypted kiosk identification to backend requests
- **Request Forwarding**: Transparently forwards authenticated requests to the backend

## Development

### Build for Development

```bash
cargo build
```

### Run in Development Mode

```bash
# Run main application
cargo run

# Run key encryption utility
cargo run --bin encrypt_key -- encrypted_blob.b64
```

### Environment Variables

| Variable | Description | Required | Default |
|----------|-------------|----------|---------|
| `ENCRYPT_PASSPHRASE` | Passphrase for AES encryption | Yes | - |
| `BACKEND_BASE_URL` | Backend API base URL | No | `http://evalify.amritanet.edu` |
| `EVALIFY_URL` | Frontend application URL | No | `http://evalify.amritanet.edu` |
| `LOCAL_AUTH_KEY` | Authentication key for proxy requests | Yes | - |
| `KIOSK_KEY` | Kiosk identification key | Yes | - |

## Security Notes

- Keep your `.env` file secure and never commit it to version control
- The `LOCAL_AUTH_KEY` should be a secure, randomly generated string
- The `KIOSK_KEY` is encrypted before transmission to the backend
- All proxy requests require proper origin and authentication headers

## Troubleshooting

### Common Issues

1. **Environment variables not found**: Ensure your `.env` file is in the project root and properly formatted

2. **Chromium not found**: Install Chromium or Google Chrome:
   ```bash
   # Ubuntu/Debian
   sudo apt install chromium-browser
   
   # Or Google Chrome
   wget -q -O - https://dl.google.com/linux/linux_signing_key.pub | sudo apt-key add -
   sudo sh -c 'echo "deb [arch=amd64] http://dl.google.com/linux/chrome/deb/ stable main" >> /etc/apt/sources.list.d/google-chrome.list'
   sudo apt update && sudo apt install google-chrome-stable
   ```

3. **Permission issues**: Ensure the binary has execute permissions:
   ```bash
   chmod +x target/release/evalify-kiosk
   chmod +x target/release/encrypt_key
   ```

4. **Port conflicts**: The proxy runs on port 8473. Ensure this port is available.

## License

Licensed under the Apache License, Version 2.0. See LICENSE file for details.
