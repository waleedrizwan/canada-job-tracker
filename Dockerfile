FROM rust:1-slim AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY static static
RUN cargo build --release --locked

# Distroless: just glibc and CA certs, no shell or package manager.
FROM gcr.io/distroless/cc-debian12
WORKDIR /app
COPY --from=build /app/target/release/canada-job-tracker /app/canada-job-tracker
COPY data /app/data
ENV PORT=8080
EXPOSE 8080
CMD ["/app/canada-job-tracker"]
