FROM docker.io/library/rust:1.56.1-alpine3.14 AS builder
COPY . /usr/src/jibri-pod-controller
RUN apk --no-cache --update add build-base libc6-compat perl \
 && cd /usr/src/jibri-pod-controller \
 && ( [ "$(apk --print-arch)" = "aarch64" ] && export CFLAGS="-mno-outline-atomics" || true ) \
 && RUSTFLAGS="-D warnings" cargo build --release --locked

FROM docker.io/library/alpine:3.14.3
RUN apk --no-cache --update add ca-certificates libc6-compat \
 && adduser --disabled-password --uid 1000 app
COPY --from=builder /usr/src/jibri-pod-controller/target/release/jibri-pod-controller /usr/local/bin/
USER app
ENTRYPOINT ["/usr/local/bin/jibri-pod-controller"]
