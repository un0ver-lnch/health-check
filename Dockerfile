FROM rust:latest AS builder

WORKDIR /workbench

COPY . .

RUN cargo build --release

FROM gcr.io/distroless/cc

WORKDIR /running

COPY --from=builder /workbench/target/release/health-check .

CMD ["./health-check"]