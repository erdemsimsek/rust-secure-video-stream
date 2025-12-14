# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A production-grade, encrypted video streaming system built in Rust. This is a learning project exploring systems programming, video processing, network security, and embedded systems. The goal is to stream video from cameras (webcam, Raspberry Pi Camera) over the network with hardware-accelerated encoding and secure transport.

**Current Status**: Phase 1 (Basic Network Streaming) - Early development with basic camera capture implemented.

## Workspace Architecture

This is a Cargo workspace with modular crate structure:

```
crates/
├── core/       # Core abstractions: Frame types, VideoSource trait, error types
├── capture/    # Camera capture implementations (V4L2, libcamera)
├── codec/      # Video encoding/decoding with GStreamer integration
├── network/    # Network transport (WebRTC/QUIC)
├── crypto/     # Security layer (TLS 1.3, certificate management)
└── ui/         # egui-based video player interface
```

**Key Design Principle**: Trait-based abstractions for hardware independence. The `VideoSource` trait allows multiple camera backends (V4L2, libcamera, mock sources) behind a unified interface.

## Build Commands

```bash
# Build entire workspace
cargo build

# Build specific crate
cargo build -p streaming-capture

# Build with release optimizations (important for performance)
cargo build --release

# Run the capture binary (currently only implemented crate)
cargo run -p streaming-capture --bin capture

# Run tests for all crates
cargo test --all

# Run tests for specific crate
cargo test -p streaming-core

# Check code without building
cargo check --all

# Run clippy lints
cargo clippy --all-targets --all-features -- -D warnings

# Format code
cargo fmt --all

# Run benchmarks (when implemented)
cargo bench
```

## Technology Stack & Rationale

### Core Technologies
- **Video Pipeline**: GStreamer (industry standard for streaming, hardware acceleration support)
- **Network Protocol**: WebRTC (built-in NAT traversal, DTLS-SRTP) or QUIC (for controlled networks)
- **Encryption**: rustls + ring (pure Rust, memory safe, TLS 1.3)
- **Async Runtime**: Tokio (ecosystem support, production maturity)
- **UI Framework**: egui + wgpu (immediate mode, GPU acceleration)

### Hardware Acceleration Strategy
Platform-specific encoders via GStreamer:
- **Raspberry Pi**: `v4l2h264enc` (V4L2 Memory-to-Memory API)
- **NVIDIA Jetson**: `nvv4l2h264enc` (NVENC)
- **Intel/AMD x86**: `vaapih264enc` / `amfh264enc`
- **Fallback**: `x264enc` (software encoding)

GStreamer automatically detects and negotiates hardware capabilities at runtime.

## Architecture Patterns

### Frame Processing Pipeline
```
Camera → Capture → Encode → Packetize → Encrypt → Network →
→ Decrypt → Depacketize → Decode → Render → Display
```

### Concurrency Model
- **Main Thread**: UI (egui event loop)
- **Capture Thread**: Blocking I/O for camera capture
- **Encoder Thread Pool**: CPU-bound encoding tasks
- **Network I/O Thread**: Tokio async runtime
- **Decoder Thread**: CPU-bound decoding
- **Render Thread**: GPU command submission

Communication via Tokio async channels with bounded capacity for backpressure.

### Error Handling
- Use `thiserror` for custom error types
- Implement graceful degradation (hardware encoder → software fallback)
- Retry with exponential backoff for transient network errors
- Never panic in production code paths

## Development Guidelines

### Working with Video Frames
- All frames use the `Frame` type from `streaming-core`
- Use `bytes::Bytes` for zero-copy buffer management
- Preserve timestamps and sequence numbers for synchronization
- Frame formats: YUYV, NV12, I420, MJPEG, H.264

### Adding New Camera Backends
1. Implement the `VideoSource` trait from `streaming-core`
2. Add platform-specific dependencies with `cfg` attributes
3. Populate `CameraCapabilities` with accurate format/resolution support
4. Use async interfaces for non-blocking capture
5. Consider MMAP buffers for zero-copy on Linux

### GStreamer Integration Notes
- Initialize GStreamer once per process: `gst::init()?`
- Use `appsrc` for pushing frames, `appsink` for receiving encoded data
- Pipeline construction should detect hardware encoders dynamically
- Configure for low latency: `tune=zerolatency`, no B-frames, short GOP
- Handle pipeline state transitions properly (Null → Ready → Playing)

### Security Requirements
- Use TLS 1.3 only (no legacy protocols)
- Implement mutual authentication with client certificates
- Certificate pinning for MITM prevention
- Use Ed25519 for certificates (modern, fast)
- AES-GCM for symmetric encryption
- Never hardcode secrets or credentials

### Performance Optimization
- **Zero-copy**: Use MMAP buffers, avoid unnecessary clones
- **Memory pooling**: Reuse frame buffers via `Arc<Mutex<FrameBuffer>>`
- **Lock-free structures**: Use atomic counters for statistics
- **Hardware acceleration**: Always prefer hardware encoders when available
- **Latency targets**: <100ms end-to-end (capture to display)

## Platform-Specific Considerations

### Raspberry Pi Development
- V4L2 is the standard camera interface
- Hardware H.264 encoder available via `v4l2h264enc`
- Limited CPU for software encoding (use hardware always)
- Cross-compilation target: `aarch64-unknown-linux-gnu` (Pi 4/Zero 2W)
- Test on actual hardware for accurate performance metrics

### Cross-compilation Setup
```bash
# Install target
rustup target add aarch64-unknown-linux-gnu

# Build for Raspberry Pi
cargo build --release --target aarch64-unknown-linux-gnu
```

## Testing Strategy

### Unit Tests
- Mock `VideoSource` implementations for testing without hardware
- Property-based testing with `proptest` for encoders
- Test error paths and edge cases

### Integration Tests
- End-to-end pipeline tests (capture → encode → network)
- Network failure simulation
- Hardware acceleration verification

### Benchmarks
- Use Criterion for performance benchmarks
- Measure encoding latency at different resolutions
- Profile with `perf` on Linux for hotspot analysis

## Common Development Tasks

### Adding a New Crate
1. Create directory in `crates/`
2. Add to workspace `members` in root `Cargo.toml`
3. Use workspace dependencies: `version.workspace = true`
4. Add inter-crate dependencies: `path = "../crate-name"`

### Debugging Camera Issues
- List available devices: `v4l2-ctl --list-devices`
- Check formats: `v4l2-ctl --device=/dev/video0 --list-formats-ext`
- Test with GStreamer: `gst-launch-1.0 v4l2src device=/dev/video0 ! autovideosink`
- Enable verbose logging: `GST_DEBUG=3 cargo run`

### Tracing and Metrics
- Use `tracing` crate for structured logging
- Instrument async functions: `#[tracing::instrument]`
- Metrics collection: `metrics` crate with Prometheus export
- Key metrics: frames captured/dropped, encode latency, buffer occupancy

## Documentation Requirements

### Code Documentation
- All public APIs must have rustdoc comments
- Include examples in doc comments
- Document error conditions
- Explain hardware-specific behavior

### Architecture Documentation
- See `ARCHITECTURE.md` for system design details
- See `TECHNOLOGY_CHOICES.md` for technology rationale
- See `IMPLEMENTATION_PHASES.md` for development roadmap
- See `PROJECT_ROADMAP.md` for timeline and goals

## Learning Resources

This project integrates multiple complex domains:
- **V4L2 API**: Direct kernel interface for video capture
- **GStreamer**: Pipeline-based multimedia framework
- **WebRTC**: Real-time communication protocol stack
- **Cryptography**: Certificate management, TLS handshake
- **Async Programming**: Tokio runtime, futures, channels
- **GPU Programming**: wgpu for video rendering

Refer to the acknowledgments in README.md for recommended learning resources.

## CI/CD

GitHub Actions workflow runs on push/PR:
- Format check: `cargo fmt --check`
- Linting: `cargo clippy` with pedantic warnings
- Tests: `cargo test --all`
- Security audit: `cargo audit`

## Performance Targets

| Metric | Target | Maximum |
|--------|--------|---------|
| End-to-end latency | 55ms | 120ms |
| Frame rate (1080p) | 30fps | 60fps |
| CPU usage (RPi 4) | 25% | 40% |
| Memory footprint | 150MB | 256MB |
| Bandwidth (1080p30) | 4Mbps | 8Mbps |

## Project Philosophy

This is a **learning project** with production-quality aspirations:
- Write clean, idiomatic Rust code
- Follow industry best practices
- Document architectural decisions
- Optimize for real-time performance
- Design for hardware constraints (embedded systems)
- Build with security from the ground up

When in doubt, prioritize correctness over performance, and clarity over cleverness.
