docker run --rm -p 8080:80 \
  -v $(pwd)/data:/data/webdav \
  cloudpoint_server
