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

# Install yt-dlp and other runtime dependencies
RUN pip install --no-cache-dir -U --pre "yt-dlp[default]"

# Set working directory
WORKDIR /app

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/app/target/release/telegram_x_video_bot .

# Copy the video directory structure (but not its contents)
RUN mkdir video

# Define a volume for the video directory to allow external access
VOLUME /app/video

# Environment variable for TELOXIDE_TOKEN (to be provided at runtime)
ENV TELOXIDE_TOKEN=""
ENV RUST_LOG=info

# Command to run the application
CMD ["./telegram_x_video_bot"]