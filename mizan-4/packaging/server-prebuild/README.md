# Mizan Server — Linux amd64 prebuild

Standalone HTTP server build for self-hosting (no Tauri, no desktop runtime).
Built on Ubuntu 22.04 (glibc 2.35) — runs on Debian 12+, Ubuntu 22.04+, and
derivatives.

## Layout

- `mizan-server` — server binary (install to `/usr/local/bin/`)
- `dist/` — frontend static assets (point `WF_STATIC_DIR` here)
- `mizan.service.example` — sample systemd unit
- `LICENSE`

## Quick start

```bash
sudo install -m 755 mizan-server /usr/local/bin/mizan-server
sudo mkdir -p /opt/mizan /opt/mizan_data
sudo cp -r dist /opt/mizan/dist

sudo tee /opt/mizan/.env >/dev/null <<EOF
WF_LISTEN_ADDR=0.0.0.0:8080
WF_DB_PATH=/opt/mizan_data/mizan.db
WF_STATIC_DIR=/opt/mizan/dist
WF_SECRET_KEY=$(openssl rand -base64 32)
WF_AUTH_PASSWORD_HASH=<argon2id hash, see docs/self-host>
# Required when auth is enabled AND you reach the server via a different
# scheme/host/port than the bind address (e.g. reverse proxy). Must match
# the URL in the browser's address bar exactly. Setting "*" is rejected.
WF_CORS_ALLOW_ORIGINS=http://<your-server-ip>:8080
EOF
sudo chmod 600 /opt/mizan/.env

sudo cp mizan.service.example /etc/systemd/system/mizan.service
sudo systemctl enable --now mizan
```

Full self-host docs: <https://github.com/afadil/mizan/blob/main/docs/self-host/>
