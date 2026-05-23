FROM rust:1.95-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim AS runtime

WORKDIR /app
COPY --from=builder /app/target/release/novellia-takehome /usr/local/bin/novellia-takehome
COPY data ./data

EXPOSE 3100

ENTRYPOINT ["novellia-takehome"]
CMD ["data/backend-takehome-fhir-resources.jsonl"]
