FROM docker.io/library/alpine:3.14.3
RUN apk --no-cache --update add openssl ca-certificates libc6-compat \
 && adduser --disabled-password --uid 1000 app
COPY target/release/jibri-pod-controller /usr/local/bin/
USER app
ENTRYPOINT ["/usr/local/bin/jibri-pod-controller"]
