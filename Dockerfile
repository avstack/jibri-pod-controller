FROM docker.io/library/rust:1.59.0-alpine3.15 AS builder
COPY . /usr/src/jibri-pod-controller
RUN apk --no-cache --update add build-base libc6-compat perl openssl-dev pkgconf \
 && cd /usr/src/jibri-pod-controller \
 && RUSTFLAGS="-D warnings" CFLAGS=$([ "$(apk --print-arch)" = "aarch64" ] && echo "-mno-outline-atomics" || echo "") cargo build --release --locked

FROM docker.io/library/alpine:3.15
RUN apk --no-cache --update add ca-certificates libc6-compat openssl \
 && adduser --disabled-password --uid 1000 app
COPY --from=builder /usr/src/jibri-pod-controller/target/release/jibri-pod-controller /usr/local/bin/
USER app
ENTRYPOINT ["/usr/local/bin/jibri-pod-controller"]
