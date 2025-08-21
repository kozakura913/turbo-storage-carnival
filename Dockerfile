FROM --platform=$BUILDPLATFORM public.ecr.aws/docker/library/rust:latest AS build_app
ARG BUILDARCH
ARG TARGETARCH
ARG TARGETVARIANT
RUN apt-get update && apt-get install -y clang musl-dev pkg-config nasm git mold
ENV CARGO_HOME=/var/cache/cargo
ENV SYSTEM_DEPS_LINK=static
COPY crossfiles /app/crossfiles
RUN bash /app/crossfiles/deps.sh
WORKDIR /app
COPY src ./src
COPY Cargo.toml ./Cargo.toml
RUN --mount=type=cache,target=/var/cache/cargo --mount=type=cache,target=/app/target bash /app/crossfiles/build.sh

FROM public.ecr.aws/docker/library/alpine:latest
ARG UID="334"
ARG GID="334"
RUN addgroup -g "${GID}" tsc && adduser -u "${UID}" -G tsc -D -h /tsc -s /bin/sh tsc
WORKDIR /tsc/
USER tsc
COPY --chown=tsc:tsc frontend /tsc/
COPY --chown=tsc:tsc --from=build_app /app/turbo-storage-carnival /tsc/turbo-storage-carnival
CMD ["/tsc/turbo-storage-carnival"]
