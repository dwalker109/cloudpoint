FROM rust:1-bullseye AS builder
WORKDIR /clpt
COPY . .
RUN --mount=type=ssh cargo build --release -p cloudpoint_server

FROM debian:bullseye-slim
COPY --from=builder /clpt/target/release/cloudpoint_server /usr/local/bin/
EXPOSE 6776
ENTRYPOINT [ "cloudpoint_server" ]
