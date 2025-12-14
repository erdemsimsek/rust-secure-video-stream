# Architecture Decisions

This document records key architectural choices, the reasoning behind them, and alternatives considered.

---

## ADR-001: Actor Pattern for Camera Control

**Date**: 2025-10-27
**Status**: Proposed
**Decision Maker**: Pending implementation

### Context

The initial `CameraInstance::start_streaming()` implementation takes ownership of `self`, making it impossible to control the camera after streaming starts. We need a way to:
1. Configure the camera
2. Start/stop streaming
3. Capture frames continuously
4. Maintain control over the camera lifecycle
5. Handle shutdown gracefully

### Problem

Rust's ownership rules prevent:
- Calling methods on `CameraInstance` after `start_streaming()` consumes it
- Sharing mutable access to camera hardware across threads
- Direct mutation of camera state from multiple contexts

### Decision

**Adopt the Actor Pattern** (also known as Active Object Pattern):
- Camera instance lives inside a dedicated thread
- All control happens via message passing (`mpsc` channels)
- Thread owns the camera exclusively (no shared mutable state)
- Commands sent in, events sent out

### Pattern Names

This approach is known by several names in different communities:
- **Actor Model** - Erlang, Akka, concurrent systems
- **Active Object Pattern** - Classical concurrency patterns
- **Message-Passing Concurrency** - General paradigm
- **Worker Thread Pattern** - Simple variation

In Rust, "Actor Pattern" is most common.

### Alternatives Considered

#### 1. Reference-Based API (`&mut self`)
```rust
impl CameraInstance {
    pub fn start_streaming(&mut self) { ... }
    pub fn stop_streaming(&mut self) { ... }
}
```
**Pros**: Simple, familiar OOP style
**Cons**: Requires locking (Mutex/RwLock), blocks on capture, hard to cancel gracefully

#### 2. Arc<Mutex<CameraInstance>>
```rust
let camera = Arc::new(Mutex::new(CameraInstance::new(...)));
let camera_clone = camera.clone();
thread::spawn(move || {
    loop {
        let frame = camera_clone.lock().unwrap().capture_frame();
    }
});
```
**Pros**: Shared ownership, multiple references
**Cons**: Lock contention, potential deadlocks, unclear ownership semantics

#### 3. Actor Pattern (Selected)
```rust
enum CameraCommand {
    Configure { width, height, fps, format },
    StartStreaming,
    StopStreaming,
    Shutdown,
}

struct CameraActor {
    camera: Camera,
    // ... state
}

fn actor_loop(
    mut actor: CameraActor,
    commands: mpsc::Receiver<CameraCommand>,
    events: mpsc::Sender<CameraEvent>,
) { ... }
```
**Pros**:
- No shared state, no locks
- Clear ownership (actor owns camera)
- Easy to reason about (sequential message processing)
- Graceful shutdown via message
- Testable (mock command/event channels)
- Idiomatic Rust for concurrent state machines

**Cons**:
- More boilerplate
- Indirection through messages (minimal overhead)
- Learning curve for pattern

### Design

#### Message Protocol

**Commands (sent TO actor)**:
```rust
pub enum CameraCommand {
    /// Discover what the camera supports
    DiscoverCapabilities,

    /// Configure camera format and resolution
    Configure {
        width: u32,
        height: u32,
        fps: u32,
        format: PixelFormat,
    },

    /// Start capturing frames
    StartStreaming,

    /// Stop capturing frames
    StopStreaming,

    /// Shutdown actor thread gracefully
    Shutdown,
}
```

**Events (sent FROM actor)**:
```rust
pub enum CameraEvent {
    /// Capabilities discovered successfully
    CapabilitiesDiscovered(CameraCapabilities),

    /// Camera configured successfully
    Configured,

    /// Frame captured
    FrameCaptured(Frame),

    /// Streaming started
    StreamingStarted,

    /// Streaming stopped
    StreamingStopped,

    /// Error occurred
    Error(CaptureError),
}
```

#### Actor Lifecycle

```
Created → Idle → Configured → Streaming → Stopped → Shutdown
   ↓                             ↓           ↓
   └─────────────────────────────┴───────────┘
```

#### Thread vs Async Task

**Decision**: Use `std::thread::spawn` (not Tokio async task)

**Reasoning**:
- `rscam::Camera::capture()` is blocking I/O
- V4L2 uses blocking `ioctl()` system calls
- No benefit from async here (no true async I/O)
- `tokio::task::spawn_blocking` would just create a thread anyway

**Exception**: The command receiver could use Tokio channels for integration with async code, but the actor loop itself runs on a regular thread.

#### Channel Configuration

**Command Channel**:
- Type: `mpsc::channel` or `tokio::mpsc::channel`
- Capacity: **Bounded (10)**
- Rationale: Commands are infrequent, bounded prevents infinite queueing

**Event Channel**:
- Type: `mpsc::channel` or `tokio::mpsc::channel`
- Capacity: **Bounded (30)** for frames, **unbounded** for control events
- Rationale: Bounded provides backpressure (if consumer can't keep up, drop frames rather than OOM)

### Implementation Checklist

- [ ] Define `CameraCommand` and `CameraEvent` enums
- [ ] Create `CameraActor` struct (wraps `Camera` + state)
- [ ] Implement `actor_loop()` function with command dispatch
- [ ] Add state machine logic (idle → configured → streaming)
- [ ] Create `CameraHandle` struct (holds command sender, join handle)
- [ ] Implement graceful shutdown with timeout
- [ ] Add tests with mock channels
- [ ] Update `main.rs` to demonstrate usage

### Example Usage

```rust
// Create actor and get handle
let (handle, events) = CameraActor::spawn("/dev/video0")?;

// Configure camera
handle.send_command(CameraCommand::Configure {
    width: 1280,
    height: 720,
    fps: 30,
    format: PixelFormat::MJPG,
})?;

// Start streaming
handle.send_command(CameraCommand::StartStreaming)?;

// Process frames
while let Some(event) = events.recv().await {
    match event {
        CameraEvent::FrameCaptured(frame) => {
            println!("Frame {}: {}x{}", frame.sequence, frame.width, frame.height);
        },
        CameraEvent::Error(e) => eprintln!("Error: {}", e),
        _ => {}
    }
}

// Graceful shutdown
handle.shutdown().await?;
```

### Learning Resources

- [Rust Atomics and Locks](https://marabos.nl/atomics/) - Chapter 4 on channels
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial/channels) - Message passing
- [Actor Pattern in Rust](https://ryhl.io/blog/actors-with-tokio/) - Alice Ryhl's blog post

---

## ADR-002: Thread vs Process for Isolation

**Date**: 2025-10-27
**Status**: Decided
**Decision Maker**: Discussion

### Context

Need to isolate camera capture operation so it doesn't block main program flow.

### Decision

**Use threads, not processes.**

### Reasoning

**Threads**:
- ✅ Shared memory space (efficient, no serialization)
- ✅ Lower overhead (microseconds to create)
- ✅ Easy message passing (channels)
- ✅ Rust prevents data races at compile time
- ✅ Appropriate for I/O isolation

**Processes**:
- ❌ Overkill for this use case
- ❌ Requires IPC (sockets, shared memory, serialization)
- ❌ Higher overhead (milliseconds to fork)
- ❌ Complex state synchronization
- ✅ Only needed for: sandboxing, hard isolation, crash recovery from unsafe code

**Conclusion**: Threads provide sufficient isolation for camera I/O without the complexity of process management.

---

## ADR-003: GStreamer for Video Pipeline (Planned)

**Date**: 2025-10-27
**Status**: Future - Not yet implemented

### Context

Need to encode captured frames to H.264 for network transmission.

### Decision

Use **GStreamer** as the video encoding pipeline.

### Reasoning

**Alternatives Considered**:
1. **FFmpeg (via ffmpeg-sys-next)** - Lower-level, more control, steeper learning curve
2. **OpenH264** - Limited, software-only, no hardware acceleration
3. **Custom codec** - Unrealistic for learning project
4. **GStreamer** ✅ - Selected

**Why GStreamer**:
- Hardware acceleration auto-detection (VAAPI, NVENC, V4L2 M2M)
- Pipeline abstraction (source → encoder → sink)
- Mature, production-grade
- Good Rust bindings (`gstreamer-rs`)
- Supports low-latency streaming
- Extensive documentation

**Trade-offs**:
- Heavier dependency (requires GStreamer system libraries)
- Slightly higher learning curve than OpenH264
- Worth it for hardware acceleration support

---

## ADR-004: Pure Event Stream (with Future Oneshot Option)

**Date**: 2025-11-01
**Status**: Decided
**Decision Maker**: Learning discussion

### Context

For the Actor pattern implementation, we need to decide how commands get responses:
1. Pure event stream - all responses come through the event channel
2. Oneshot channels - each command carries a private response channel
3. Hybrid - mix of both approaches

### Decision

**Start with Pure Event Stream**

Commands and events both go through their respective channels. Clients handle matching events to commands they sent.

### Reasoning

**Why pure event stream first:**
- ✅ Simpler to understand and implement
- ✅ Less boilerplate code
- ✅ Good enough for sequential command patterns
- ✅ Can refactor to oneshot later if needed

**Why not oneshot (yet):**
- More complex to understand initially
- Extra boilerplate for each request (create channel, pass sender, await receiver)
- Overkill for simple sequential operations

### Future Enhancement: Oneshot Channels

**When to add oneshot:**
- If we need concurrent requests (send multiple commands without waiting)
- If event matching becomes complex (tracking which response is for which command)
- When learning about request-response patterns in Rust

**How to implement:**
```rust
use tokio::sync::oneshot;

pub enum CameraCommand {
    Configure {
        width: u32,
        height: u32,
        respond_to: oneshot::Sender<Result<(), CaptureError>>
    },
    // ... other commands
}
```

**Usage:**
```rust
let (tx, rx) = oneshot::channel();
handle.send(CameraCommand::Configure {
    width: 1280,
    height: 720,
    respond_to: tx
})?;
let result = rx.await?; // Direct response!
```

### Implementation Note

Keep this option in mind. If client code starts looking like a complex state machine to track "which event is for which command", that's the signal to refactor to oneshot.

---

## Template for Future Decisions

```markdown
## ADR-XXX: Title

**Date**: YYYY-MM-DD
**Status**: Proposed | Decided | Deprecated
**Decision Maker**: Name or "Team"

### Context
What problem are we solving?

### Decision
What did we decide?

### Alternatives Considered
What else did we evaluate?

### Reasoning
Why this choice?

### Consequences
What are the implications?

### Implementation Notes
Gotchas, tips, references
```

---

**Note**: Update this file whenever making significant architectural choices.
