FROM ubuntu:24.04

WORKDIR /nillion

# We need the PKI roots to be installed for certificates to be validated.
RUN apt update \
  && apt install -y ca-certificates \
  && apt clean

COPY functional-tests /nillion/
COPY cargo2junit /usr/local/bin/

CMD ["/nillion/functional-tests"]
