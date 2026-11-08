FROM rust:1-bookworm AS build
WORKDIR /src
COPY core core
COPY server server
COPY godot/levels godot/levels
RUN cd server && cargo build --release
FROM debian:bookworm-slim
COPY --from=build /src/server/target/release/dbserver /usr/local/bin/dbserver
COPY --from=build /src/godot/levels /levels
VOLUME /data
EXPOSE 8787
CMD ["dbserver", "--bind", "0.0.0.0:8787", "--db", "/data/downbeat.db", "--levels", "/levels"]
