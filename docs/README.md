# Rust Secure Streaming

> **Note**: This is a learning project where I'm exploring systems programming, video processing, and network security in Rust. The goal is to build a production-quality encrypted video streaming system while mastering Rust and industry-standard technologies.

## Overview

A real-time video streaming application with end-to-end encryption, built in Rust. This project streams video from cameras (webcam, Raspberry Pi Camera) over the network with hardware-accelerated encoding and secure transport.

## Learning Goals

This project is my journey to deeply understand:
- **Systems Programming**: Low-level video capture (V4L2), zero-copy buffers, hardware acceleration
- **Rust Mastery**: Async/await, lifetimes, trait design, unsafe code
- **Video Processing**: GStreamer pipelines, H.264 encoding, color space conversion
- **Network Protocols**: TCP, QUIC, WebRTC, and their trade-offs
- **Cryptography**: TLS 1.3, certificate management, SRTP
- **Embedded Systems**: Cross-compilation, hardware optimization on Raspberry Pi

## Current Status

**Phase 1: Basic Network Streaming** (In Progress)
- [x] Camera capture with rscam
- [ ] TCP-based frame transmission
- [ ] egui-based video player
- [ ] Basic frame synchronization

**Upcoming Phases:**
- Phase 2: H.264 encoding with GStreamer
- Phase 3: TLS encryption with rustls
- Phase 4: Raspberry Pi hardware acceleration
- Phase 5: Edge AI integration (Jetson Nano)

## Tech Stack

- **Language**: Rust 2021
- **Video Capture**: rscam (V4L2 wrapper)
- **Async Runtime**: Tokio
- **UI**: egui with wgpu backend
- **Planned**: GStreamer, WebRTC/QUIC, rustls

## Hardware Tested

- Ubuntu laptop with integrated webcam
- Raspberry Pi 4B
- Raspberry Pi Zero 2W
- (Future: NVIDIA Jetson Nano for AI)

## Project Structure

```
camera_view/
├── src/
│   ├── sender.rs       # Camera capture + network sender
│   ├── receiver.rs     # Network receiver + video display
│   └── main.rs         # Shared utilities
├── docs/               # Architecture docs and planning
└── examples/           # Usage examples
```

## Building

```bash
# Build sender
cargo build --bin sender --release

# Build receiver
cargo build --bin receiver --release
```

## Running

```bash
# Terminal 1: Start receiver
cargo run --bin receiver

# Terminal 2: Start sender
cargo run --bin sender
```

## Documentation

See [ARCHITECTURE.md](ARCHITECTURE.md) for system design details and [IMPLEMENTATION_PHASES.md](IMPLEMENTATION_PHASES.md) for the development roadmap.

## Contributing

This is primarily a personal learning project, but suggestions and feedback are welcome! Feel free to open issues with:
- Architecture improvements
- Code review feedback
- Learning resources
- Bug reports

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

## Acknowledgments

Learning from these excellent resources:
- [GStreamer Rust bindings](https://gitlab.freedesktop.org/gstreamer/gstreamer-rs)
- [Tokio async runtime](https://tokio.rs/)
- [egui immediate mode GUI](https://github.com/emilk/egui)
- WebRTC in Rust community

---

**Status**: 🚧 Active Development | **Timeline**: 12 weeks | **Progress**: Week 1
