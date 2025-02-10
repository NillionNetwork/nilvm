# nilup

Tool to manage nillion SDK versions.

It supports a global version (`nilup use <version>`), also allows setting the version based on a file (`nil-sdk.toml`)
in the current path or parent paths ala `rust-toolchain.toml`, and also supports setting the version as an
argument `nillion +<version>`.
Part of the magic is done using nilup as a wrapper of the other sdk commands so when they get invoked nilup checks if
the required version is installed, if not, installs it, and then calls the correct version of the tool.