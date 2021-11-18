FROM docker.io/library/rust:1.56.1-alpine3.14 AS builder
COPY . .
COPY ./entrypoint.sh /
RUN apk --update add build-base openssl-dev libc6-compat \
 && RUSTFLAGS="-D warnings" cargo build --release --locked

FROM docker.io/library/alpine:3.14.3
RUN apk --update add openssl ca-certificates libc6-compat bash\
 && adduser --disabled-password --uid 1000 app
COPY --from=builder target/release/jibri-pod-controller /usr/local/bin/
COPY --from=builder entrypoint.sh /
RUN chmod 777 entrypoint.sh
USER app
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/jibri-pod-controller"]
