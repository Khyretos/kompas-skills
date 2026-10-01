# Kreative Kompanion server + web app in one small image.

FROM node:22-alpine AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run typecheck && npm run build

FROM rust:1-alpine AS server
RUN apk add --no-cache musl-dev
WORKDIR /src/server
COPY server/ ./
RUN cargo build --release --locked

FROM alpine:3
RUN apk add --no-cache ca-certificates && adduser -D -H -u 10001 kompanion && mkdir /data && chown kompanion /data
COPY --from=server /src/server/target/release/kompanion-server /usr/local/bin/kompanion-server
COPY --from=web /src/web/dist /app/web
USER kompanion
ENV KOMPANION_CONFIG=/config/kompanion.toml
VOLUME /data
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/kompanion-server"]
