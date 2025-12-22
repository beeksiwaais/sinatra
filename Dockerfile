# Builder stage
FROM rust:1-slim-bookworm as builder

WORKDIR /usr/src/app
COPY . .

# Install dependencies for ffmpeg-next (which links to system ffmpeg libraries)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libavutil-dev \
    libavformat-dev \
    libavfilter-dev \
    libavdevice-dev \
    clang \
    && rm -rf /var/lib/apt/lists/*

RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /usr/local/bin

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ffmpeg \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/sinatra .

ENV UPLOAD_DIR=/app/uploads
ENV ADDR=0.0.0.0
RUN mkdir -p ${UPLOAD_DIR}

EXPOSE 3000

CMD ["./sinatra"]
