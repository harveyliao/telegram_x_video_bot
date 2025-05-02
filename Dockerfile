# Use a multi-stage build to keep the final image lean
# Stage 1: Build the Rust application
FROM rust:1.82 AS builder

# Set working directory
WORKDIR /app

# Copy Cargo files and create a dummy main.rs to cache dependencies
COPY Cargo.toml ./
RUN mkdir src && echo "fn main() { println!(\"Dummy\"); }" > src/main.rs
RUN cargo build --release

# Copy the actual source code and build the application
COPY src ./src
RUN cargo build --release --bin telegram_x_video_bot

# Stage 2: Create the runtime image
FROM debian:bookworm-slim

# Set working directory
WORKDIR /app

# Install yt-dlp and OpenSSL lib
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        python3 \
        ffmpeg \
        openssl \
        ca-certificates && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Install yt-dlp
RUN pip install --no-cache-dir -U --pre "yt-dlp[default]"

# Create non-root user
RUN adduser --disabled-password --gecos '' botuser
USER botuser
ENV HOME=/home/botuser

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/telegram_x_video_bot ./
copy --chown=botuser:botuser twitter.txt ./

# Copy the video directory structure (but not its contents)
RUN mkdir video

# Define a volume for the video directory to allow external access
VOLUME /app/video

# Command to run the application
CMD ["./telegram_x_video_bot"]
