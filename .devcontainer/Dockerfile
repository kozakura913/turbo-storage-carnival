FROM public.ecr.aws/docker/library/rust:latest as build_app
RUN apt-get update && apt-get install -y clang musl-dev pkg-config nasm git mold
RUN mkdir /musl && curl -sSL https://github.com/kozakura913/musl.cc/releases/download/20211122/x86_64-linux-musl-cross.tgz | tar -zxf - -C /musl
RUN apt-get install -y ffmpeg
ARG UID="991"
ARG GID="991"
RUN groupadd -g "${GID}" tsc && useradd -l -u "${UID}" -g "${GID}" -m -d /tsc tsc
WORKDIR /tsc/
USER tsc
ENV CARGO_HOME=/var/cache/cargo
ENV MUSL_NAME="x86_64-linux-musl-cross"
ENV PATH="/musl/x86_64-linux-musl-cross/bin:${PATH}"
ENV CC=x86_64-linux-musl-gcc
ENV CXX=x86_64-linux-musl-g++
ENV AR=x86_64-linux-musl-ar
ENV RUSTFLAGS="-C target-cpu=x86-64-v3 -C linker=clang -C link-arg=-fuse-ld=mold"
ENV PKG_CONFIG_SYSROOT_DIR="/musl/x86_64-linux-musl-cross/"
ENV RUST_TARGET="x86_64-unknown-linux-musl"
ENV BINDGEN_EXTRA_CLANG_ARGS="--sysroot /musl/x86_64-linux-musl-cross/x86_64-linux-musl"
ENV CARGO_BUILD_TARGET=x86_64-unknown-linux-musl
CMD ["bash"]
