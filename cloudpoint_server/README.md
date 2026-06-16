# Cloudpoint Server 1.*.*

This crate contains the server powering Cloudpoint. It is used for the
public instance at https://cloudpoint.dwalker.me and you can also self
host it. It is available as a docker image and compose files are provided
to make it very easy.

## Building

**Soon** you will be able to build the server as a static binary and run it
directly, if that's what you prefer. You can't do it yet, since it relies 
on another one of my crates which isn't yet public. Sorry about that; it
will be done soon.

## Docker

### Local network

`compose.local.yml` is what most homelabs should use. It includes 
`dwalker109/cloudpoint` and `postgres` and should work out of the box:

`docker compose -f compose.local.yml up` is all you need. If you want
to customise ports, passwords etc just take a look in that file. If you
want to introduce a reverse proxy or tailscale or similar, you can 
add it here.

This version does NOT use TLS by default so **should not** be exposed
to the internet - just stick to your local network.

### Public network (i.e. internet)

`compose.public.yml` introduces `caddy` as a reverse proxy. This allows
TLS to be used with near zero configuration. It also avoids publishing
any container ports, other than via caddy. If you want to run your own
publicly accessible instance, this is the compose file to use.

You will need a domain pointing at your server for this one. Add it to
`Caddyfile` and then run `docker compose -f compose.public.yml up`. 
Again, if you want to customise ports, passwords etc just take a 
look in that file.

## Upgrading from Cloudpoint Server 0.*.*

Users of the beta version of Cloudpoint might have the old DUFS based
server running. This kept all your data in files on the filesystem.
The simplicity was nice but crippling once you had a good number of 
save versions, so has been replaced. Stop your old compose setup and
switch to the new one.

**Optionally**, you can import your old save data into the new version.
You don't really need to - you can sync your consoles and get everything
back again quite easily - but if you prefer to import, you can run this:

`docker compose -f compose.local.yml run --rm -v <ABSOLUTE PATH TO DATA DIR>:/import-data cloudpoint import-v0 /import-data`

Replace `<ABSOLUTE PATH TO DATA DIR>` with whatever you previously set 
your DUFS directory to. It should contain a directory called `sync`.
