FROM --platform=$BUILDPLATFORM alpine

ARG BINARY_PATH

COPY ${BINARY_PATH} /usr/local/bin/mqdish-consumer

ENTRYPOINT ["/usr/local/bin/mqdish-consumer"]