# Self-Hosting Mizan

Mizan ships an official Docker image so you can run the web edition on your own
hardware. Full self-hosting guides live on the website:

📘
**[mizan.app/docs/guide/self-hosting](https://mizan.app/docs/guide/self-hosting)**

This directory only holds in-repo artifacts (the Unraid CA template) and short
pointers per platform.

## Image

Multi-arch (`linux/amd64`, `linux/arm64`), published on every `v*.*.*` tag:

| Registry   | Image                         |
| ---------- | ----------------------------- |
| Docker Hub | `afadil/mizan:latest`         |
| GHCR       | `ghcr.io/afadil/mizan:latest` |

```bash
docker pull afadil/mizan:latest
```

## Platform pointers

- [**Docker / Docker Compose**](https://mizan.app/docs/guide/self-hosting): the
  canonical path. Full walkthrough on the website.
- [**Unraid**](./unraid/): install via Community Apps. The CA template lives in
  this repo at [`unraid/template.xml`](./unraid/template.xml).
- [**Proxmox VE**](./proxmox/): LXC via community-scripts, Docker-in-LXC, or
  Docker VM.
