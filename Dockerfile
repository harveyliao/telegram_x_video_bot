# Use a multi-stage build to keep the final image lean
# Stage 1: Build the Rust application
FROM rust:1.82 AS builder

# Set working directory
WORKDIR /usr/src/app

# Copy Cargo files and create a dummy main.rs to cache dependencies
COPY Cargo.toml ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy the actual source code and build the application
COPY src ./src
RUN cargo build --release

# Stage 2: Create the runtime image
FROM python:3.12-slim

# Install runtime dependencies: yt-dlp needs Python, Teloxide needs OpenSSL
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN pip install --no-cache-dir -U --pre "yt-dlp[default]"

# Create a non-root user and group
RUN groupadd --system --gid 1001 appuser && \
    useradd --system --uid 1001 --gid appuser --shell /bin/bash --create-home appuser

# Set working directory
WORKDIR /app

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/app/target/release/telegram_x_video_bot .

# Create the video directory and set permissions
RUN mkdir video && chown -R appuser:appuser /app
# Set permissions for the binary
RUN chown appuser:appuser /app/telegram_x_video_bot

# Switch to the non-root user
USER appuser

# Define a volume for the video directory to allow external access
VOLUME /app/video

# Command to run the application
CMD ["./telegram_x_video_bot"]
