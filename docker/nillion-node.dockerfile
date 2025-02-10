FROM debian:bookworm-20240926-slim AS ca-certificates
RUN apt update && \
    apt install -y ca-certificates && \
    update-ca-certificates

FROM scratch
COPY --from=ca-certificates /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY node /node
CMD [ "/node" ]
