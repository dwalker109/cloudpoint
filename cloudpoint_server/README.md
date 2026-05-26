# Cloudpoint Server

Cloudpoint simply uses a few open source tools strung together to provide a server. 
There isn't any custom code here at all, which is by design. The tools used are:

- [DUFS](https://github.com/sigoden/dufs)
- [Caddy](https://github.com/caddyserver/caddy)
- [Docker](https://github.com/docker)

## Self Hosting

All you need is a Linux server running Docker. macOS is probably also fine. Maybe
Windows, even; I don't know but if you do please let me know.

1. Copy the contents of this directory to your server.
2. If you intend to use the prod version (which supports TLS) set your domain
   in Caddyfile.prod
3. Run `docker compose -f compose.dev.yml up` (HTTP on port 80 only, suitable
   for running on your private local network) or 
   `docker compose -f compose.prod.yml up` (HTTPS on port 443, will obtain a
   TLS cert automatically, suitable for running on the public internet).
4. On your 3DS consoles, create `settings.ini` and add:
   `base_url = your_fully_qualified_domain` (e.g. `http://192.168.1.1`)

Your consoles will now use your self hosted server instead of the public one.
