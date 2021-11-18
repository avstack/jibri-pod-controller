FROM docker.io/library/rust:1.56.1-alpine3.14 AS builder
COPY . .
RUN apk --no-cache --update add build-base openssl-dev libc6-compat \
 && RUSTFLAGS="-D warnings" cargo build --release --locked

FROM docker.io/library/alpine:3.14.3
RUN apk --no-cache --update add openssl ca-certificates libc6-compat \
 && adduser --disabled-password --uid 1000 app
COPY --from=builder target/release/jibri-pod-controller /usr/local/bin/
USER app
ENTRYPOINT ["/usr/local/bin/jibri-pod-controller"]
