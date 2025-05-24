# 1. Change the base image to rust:latest
FROM rust:latest

# 2. Install FFmpeg development libraries
RUN apt-get update && \
    apt-get install -y libavutil-dev libavformat-dev libavcodec-dev libswscale-dev libswresample-dev pkg-config && \
    rm -rf /var/lib/apt/lists/*

# 3. Set the working directory
WORKDIR /usr/src/app

# 4. Copy project files
COPY . .

# 5. Build the Rust application in release mode
RUN cargo build --release

# 6. Set the CMD to execute the compiled application
CMD ["/usr/src/app/target/release/sinatra"]
