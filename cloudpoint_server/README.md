# Cloudpoint Server

Cloudpoint simply uses a few open source tools strung together to provide a server. 
There isn't any custom code here at all, which is by design. The tools used are:

- [DUFS](https://github.com/sigoden/dufs)
- [Caddy](https://github.com/caddyserver/caddy)
- [Docker](https://github.com/docker)

## Self Hosting

All you need is a Linux server running Docker. macOS is probably also fine. Maybe
Windows, even; I don't know but if you do please let me know.

### For a private network, not accessible from the internet

1. Copy `Caddyfile.dev` and `compose.dev.yml` to your server.
3. Run `docker compose -f compose.dev.yml up` (HTTP on port 80 only).
4. On your 3DS consoles, create `/3ds/Cloudpoint/settings.ini` and add:
   `base_url = http://<server_ip>` (e.g. `http://192.168.1.1`)

Your consoles will now use your self hosted server instead of the public one.

### For a public network, accessible from the internet

1. Copy `Caddyfile.prod` and `compose.prod.yml` to your server.
2. Update `compose.prod.yml` with your domain on the first line. 
3. Run `docker compose -f compose.prod.yml up` (HTTPS on port 443 only, TLS
   configured with a domain cert automaticaly via LetsEncrypt).
4. On your 3DS consoles, create `/3ds/Cloudpoint/settings.ini` and add:
   `base_url = https://<fqdn>` (e.g. `https://my-public-cloud.me`)

## Troubleshooting

Some users have reported issues when self hosting, such as 404 errors, 
necessary directories not being created, and general inability to sync.
There are a lot of factors which could affect this due to the wide range
of self hosting options available to you, so here are some troubleshooting 
steps gathered from https://github.com/dwalker109/cloudpoint/issues/86

1. If possible, stick to the provided DUFS and Caddy containers; you could
   add on additional components (such as NGINX proxy manager) as well, but
   the basic defaults will usually help (and overhead on Linux is negligable).
2. If you remove or change things, make sure the correct DUFS port is made 
   available over the network - either the default 5000, or whatever port
   your expose it on through your docker setup.
3. Make sure the user which runs your containers really has perms to write 
   to your storage location - this isn't always obvious on systems like
   TrueNAS or standalone NAS servers.
4. Some users have reported needing to tweak the DUFS config to get things
   working. I'm not sure why this would have helped, but I presume some
   combination of hosting setup and DUFS created breakage which was resolved
   by making DUFS more permissive. **Only change these settings if your
   server is locked inside your private network - do not change these for
   a publicly accessible server!**:
   - Try setting `allow-all: true`
   - Try removing the `hidden` settings
