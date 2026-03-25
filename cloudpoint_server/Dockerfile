FROM openresty/openresty:bookworm

RUN mkdir -p /data/webdav /data/webdav_tmp && chown -R nobody:nogroup /data/webdav /data/webdav_tmp

COPY nginx-webdav.conf /usr/local/openresty/nginx/conf/nginx.conf

VOLUME ["/data/webdav"]

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
