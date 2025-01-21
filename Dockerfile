FROM rust:latest AS builder
WORKDIR /workbench
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libsqlite3-0 libc6 curl
WORKDIR /running
COPY --from=builder /workbench/target/release/health-check .
CMD ["./health-check"]