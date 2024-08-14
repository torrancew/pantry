FROM docker.io/library/rust:1-slim-bookworm as builder

RUN apt-get update && apt-get install -y build-essential gcc g++ clang-16 libc++1-16 libclang1-16 libxapian-dev && apt-get clean -y
WORKDIR /usr/src/pantry
COPY . .
RUN env cargo install --path .

FROM docker.io/library/debian:bookworm-slim
WORKDIR /recipes
RUN apt-get update && apt-get install -y libxapian30 && apt-get clean -y
COPY --from=builder /usr/local/cargo/bin/pantry /usr/local/bin/pantry

EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/pantry"]
CMD ["--listen-on", "0.0.0.0:3000", "--recipe-dir", "/recipes"]
