FROM python:3.10.4-bullseye

SHELL ["/bin/bash", "-c"]

COPY ./tools/load-tool/reports/ generator
WORKDIR generator

RUN ./scripts/build_virtualenv.sh
ENTRYPOINT [ "./scripts/generate_report.sh" ]
