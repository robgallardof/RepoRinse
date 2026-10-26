# syntax=docker/dockerfile:1
# Optional container for the reporinse CLI: run `check` / `fix` on a mounted repository
# (for CI or machines without Rust). Git hooks and agent integrations are installed
# natively, because a hook installed from a container would point at a container path.
FROM rust:1-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY docs/rules ./docs/rules
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --locked --release && cp target/release/reporinse /build/reporinse

FROM debian:bookworm-slim
# A bind-mounted repository usually belongs to another UID: trust only /repo, in the
# image's own system Git config (nothing is written on the host).
RUN apt-get update && apt-get install -y --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 reporinse && useradd --uid 10001 --gid reporinse --create-home reporinse \
    && mkdir /repo && chown reporinse:reporinse /repo \
    && git config --system --add safe.directory /repo
COPY --from=build /build/reporinse /usr/local/bin/reporinse
COPY LICENSE /usr/share/doc/reporinse/LICENSE
ENV NO_COLOR=1
USER reporinse
WORKDIR /repo
ENTRYPOINT ["reporinse"]
CMD ["--help"]
