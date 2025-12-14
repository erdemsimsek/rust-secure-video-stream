# System Architecture - Secure Video Streaming Platform

## Architecture Overview

This document describes the production-grade architecture for a secure, real-time video streaming system built with Rust. The design prioritizes modularity, performance, and security while maintaining clean separation of concerns.

## System Components

### 1. Capture Layer
```
┌─────────────────────────────────────────────┐
│              Capture Abstraction             │
├─────────────────────────────────────────────┤
│ trait VideoSource {                          │
│   async fn capture_frame() -> Frame;         │
│   fn capabilities() -> CameraCapabilities;   │
│   fn configure(config: CaptureConfig);       │
│ }                                            │
├─────────────────────────────────────────────┤
│   V4L2Source  │  LibcameraSource  │  Mock   │
└─────────────────────────────────────────────┘
```

**Key Design Decisions:**
- Trait-based abstraction for multiple camera backends
- Async interface for non-blocking capture
- Zero-copy frame buffers using `bytes::Bytes`
- Hardware format negotiation (YUYV, MJPEG, H.264)

### 2. Video Processing Pipeline
```
┌────────────────────────────────────────────────────┐
│                 GStreamer Pipeline                  │
├────────────────────────────────────────────────────┤
│  appsrc → videoconvert → x264enc → h264parse →     │
│  → rtph264pay → appsink                            │
├────────────────────────────────────────────────────┤
│  Hardware Acceleration Variants:                    │
│  - RPi: v4l2h264enc (V4L2 M2M)                     │
│  - Jetson: nvv4l2h264enc (NVENC)                   │
│  - Intel: vaapih264enc (VAAPI)                     │
└────────────────────────────────────────────────────┘
```

**Pipeline Strategy:**
- Dynamic pipeline construction based on hardware
- Capability detection at runtime
- Fallback to software encoding
- Quality/latency trade-off profiles

### 3. Network Transport Architecture

#### Option A: WebRTC (Recommended for Production)
```
┌─────────────────────────────────────────┐
│           WebRTC Stack                    │
├─────────────────────────────────────────┤
│  ICE/STUN/TURN for NAT Traversal         │
│  DTLS-SRTP for Media Encryption          │
│  SCTP for Data Channel                   │
│  Automatic Bandwidth Adaptation          │
└─────────────────────────────────────────┘
```

**Implementation:** `webrtc-rs` or `str0m` crate

#### Option B: Custom Protocol over QUIC
```
┌─────────────────────────────────────────┐
│         Custom QUIC Protocol              │
├─────────────────────────────────────────┤
│  Control Stream (0): Signaling           │
│  Video Stream (1): H.264 NAL Units       │
│  Audio Stream (2): Optional Opus         │
│  Data Stream (3): Metadata/Telemetry     │
└─────────────────────────────────────────┘
```

**Implementation:** `quinn` with custom protocol

### 4. Security Architecture

```
┌──────────────────────────────────────────────────┐
│              Security Layer                       │
├──────────────────────────────────────────────────┤
│  Certificate Authority (CA)                       │
│  ├── Root CA (offline, Ed25519)                  │
│  ├── Intermediate CA (online, Ed25519)           │
│  └── Device Certificates (ECDSA P-256)           │
├──────────────────────────────────────────────────┤
│  Mutual TLS 1.3 Authentication                    │
│  ├── Client Certificate Verification              │
│  ├── Certificate Pinning                          │
│  └── OCSP Stapling                               │
├──────────────────────────────────────────────────┤
│  Media Encryption                                 │
│  ├── SRTP with AES-GCM-256                       │
│  ├── Key Rotation every 2^31 packets             │
│  └── Forward Secrecy via ECDHE                   │
└──────────────────────────────────────────────────┘
```

**Key Management:**
- Hardware security module (HSM) support via PKCS#11
- Secure key storage using OS keyring
- Automatic certificate renewal
- Revocation via CRL/OCSP

### 5. Application Layer

```
┌────────────────────────────────────────────┐
│           egui Application                  │
├────────────────────────────────────────────┤
│  ┌──────────────────────────────────────┐  │
│  │     Video Rendering (wgpu)           │  │
│  │  - YUV to RGB conversion on GPU      │  │
│  │  - Bilinear scaling                  │  │
│  │  - Frame timing/synchronization      │  │
│  └──────────────────────────────────────┘  │
│  ┌──────────────────────────────────────┐  │
│  │     Control Panel                    │  │
│  │  - Connection management             │  │
│  │  - Quality settings                  │  │
│  │  - Statistics overlay               │  │
│  └──────────────────────────────────────┘  │
└────────────────────────────────────────────┘
```

## Data Flow

### Streaming Pipeline
```
Camera → Capture → Encode → Packetize → Encrypt → Network → 
→ Decrypt → Depacketize → Decode → Render → Display
```

### Control Flow
```
UI Event → Command → Controller → State Machine → 
→ Pipeline Reconfiguration → Status Update → UI Refresh
```

## Concurrency Model

### Thread Architecture
```
Main Thread (UI)
├── Capture Thread (blocking I/O)
├── Encoder Thread Pool (CPU-bound)
├── Network I/O Thread (Tokio runtime)
├── Decoder Thread (CPU-bound)
└── Render Thread (GPU commands)
```

### Channel Communication
```rust
// Frame flow using async channels
let (frame_tx, frame_rx) = tokio::sync::mpsc::channel::<Frame>(10);
let (encoded_tx, encoded_rx) = tokio::sync::mpsc::channel::<EncodedFrame>(30);
let (network_tx, network_rx) = tokio::sync::mpsc::channel::<NetworkPacket>(100);
```

### Synchronization
- Lock-free ring buffers for frame queues
- Atomic counters for statistics
- RwLock for configuration changes
- Condition variables for frame sync

## Performance Optimization

### Memory Management
```rust
// Zero-copy frame buffer pool
pub struct FramePool {
    frames: Vec<Arc<Mutex<FrameBuffer>>>,
    available: crossbeam::channel::Sender<Arc<Mutex<FrameBuffer>>>,
}

// Direct memory mapping for capture
let mmap_buffer = unsafe {
    memmap2::MmapMut::map_mut(&device_fd)?
};
```

### Hardware Acceleration Points
1. **Capture**: DMA buffers with V4L2
2. **Encoding**: Hardware encoders (V4L2 M2M, NVENC, VAAPI)
3. **Color Conversion**: GPU shaders or hardware VPU
4. **Encryption**: AES-NI instructions
5. **Rendering**: GPU texture upload via wgpu

### Latency Optimization
- **Capture**: Use MMAP with multiple buffers
- **Encoding**: Tune for low-latency (no B-frames, short GOP)
- **Network**: Disable Nagle's algorithm, use jumbo frames
- **Decoding**: Start decoding before full frame arrival
- **Rendering**: Triple buffering with frame pacing

## Error Handling Strategy

### Resilience Patterns
```rust
// Retry with exponential backoff
async fn connect_with_retry(endpoint: &str) -> Result<Connection> {
    let mut backoff = ExponentialBackoff::default();
    loop {
        match connect(endpoint).await {
            Ok(conn) => return Ok(conn),
            Err(e) if e.is_transient() => {
                let delay = backoff.next_backoff()
                    .ok_or(Error::MaxRetriesExceeded)?;
                tokio::time::sleep(delay).await;
            }
            Err(e) => return Err(e),
        }
    }
}
```

### Graceful Degradation
1. Hardware encoder fails → Fall back to software
2. High-resolution fails → Drop to lower resolution
3. Network congestion → Reduce bitrate/framerate
4. Decoder overload → Skip frames
5. GPU unavailable → Software rendering

## Monitoring & Observability

### Metrics Collection
```rust
// Using the metrics crate
metrics::counter!("frames_captured", 1);
metrics::histogram!("encode_latency_ms", encode_time.as_millis() as f64);
metrics::gauge!("buffer_occupancy", queue.len() as f64);
```

### Tracing
```rust
#[tracing::instrument(level = "debug", skip(frame))]
async fn process_frame(frame: Frame) -> Result<EncodedFrame> {
    let span = tracing::span!(Level::DEBUG, "encode_frame");
    let _enter = span.enter();
    // Processing logic
}
```

### Health Checks
- Pipeline state monitoring
- Frame rate analysis
- Latency measurements
- Packet loss detection
- CPU/Memory usage tracking

## Scalability Considerations

### Multi-Stream Support
```rust
pub struct StreamManager {
    streams: DashMap<StreamId, StreamContext>,
    max_streams: usize,
    resource_pool: Arc<ResourcePool>,
}
```

### Load Balancing
- Round-robin frame distribution to encoder threads
- Priority queues for real-time streams
- Admission control based on resources

### Future Extensions
1. **SFU Mode**: Selective forwarding for multi-party
2. **Recording**: Simultaneous streaming and recording
3. **Transcoding**: Multiple quality levels
4. **CDN Integration**: HLS/DASH output
5. **Cloud Relay**: Hybrid edge/cloud architecture

## Security Considerations

### Threat Model
- **Network Attackers**: Prevented by TLS 1.3
- **MITM Attacks**: Mitigated by certificate pinning
- **Replay Attacks**: Prevented by SRTP sequence numbers
- **Resource Exhaustion**: Rate limiting and quotas
- **Side Channels**: Constant-time crypto operations

### Compliance
- **GDPR**: Encryption at rest and in transit
- **CCPA**: User consent and data deletion
- **HIPAA**: Audit logs and access control (if applicable)

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_frame_capture() {
        let source = MockVideoSource::new();
        let frame = source.capture_frame().await.unwrap();
        assert_eq!(frame.format(), PixelFormat::YUYV);
    }
}
```

### Integration Tests
- End-to-end streaming tests
- Network failure simulation
- Hardware acceleration verification
- Security penetration tests

### Performance Tests
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_encode(c: &mut Criterion) {
    c.bench_function("h264_encode", |b| {
        b.iter(|| encode_frame(black_box(test_frame())))
    });
}
```

## Deployment Architecture

### Container Strategy
```dockerfile
# Multi-stage build for minimal image
FROM rust:1.75 as builder
# Build steps...

FROM debian:bookworm-slim
# Runtime with only necessary libraries
```

### Orchestration
```yaml
# Kubernetes deployment
apiVersion: apps/v1
kind: Deployment
metadata:
  name: streaming-server
spec:
  replicas: 3
  template:
    spec:
      containers:
      - name: streamer
        resources:
          requests:
            memory: "256Mi"
            cpu: "500m"
          limits:
            memory: "512Mi"
            cpu: "2000m"
```

---

*This architecture provides a solid foundation for a production-grade streaming system while remaining flexible enough for experimentation and learning.*
