FROM scratch
COPY load-tool /load-tool
COPY programs /programs
ENTRYPOINT [ "/load-tool" ]
