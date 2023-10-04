FROM rust:1.68
COPY ../proxy .
RUN cargo build --release
EXPOSE 3333