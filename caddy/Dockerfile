FROM juvenn/caddy-dav:2.10.2

# Copy Caddyfile
COPY Caddyfile /etc/caddy/Caddyfile

# Data directory for uploads
RUN mkdir -p /data

EXPOSE 8080

CMD ["caddy", "run", "--config", "/etc/caddy/Caddyfile", "--adapter", "caddyfile"]
