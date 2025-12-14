# Current Project State

**Last Updated**: 2025-11-26

## Phase Status

**Current Phase**: Phase 1 Complete → Starting Phase 2 (H.264 Encoding)

**Current Focus**: Implementing H.264 video encoding with GStreamer

## What's Implemented

### `crates/core/` - Core Abstractions ✅
- ✅ `Frame` struct with timestamp, sequence, pixel data
- ✅ `PixelFormat` enum (MJPEG, YUYV, RGB3, BGR3, YU12, YV12)
- ✅ FourCC conversion (V4L2 ↔ enum)
- ✅ `Resolution` struct
- ✅ `CameraCapabilities` and `FormatCapability` for hardware discovery

### `crates/capture/` - Camera Capture ✅
- ✅ **Actor Pattern Implementation** - Complete camera control system
- ✅ `CameraActor` with state machine (Idle → Configured → Streaming)
- ✅ `CameraHandle` for thread-safe camera control
- ✅ Command/Event system for async communication
- ✅ Camera discovery (`discover_cameras()`)
- ✅ Capability discovery (format/resolution detection)
- ✅ Frame capture with timestamps and sequence numbers
- ✅ **Race condition fix** - State updated before camera.stop()
- ✅ Graceful shutdown handling
- ✅ Smart event loop (blocks when idle, polls during streaming)

### `crates/ui/` - Video Viewer UI ✅
- ✅ **Full egui-based application** with real-time video display
- ✅ Background frame decoding (async task in Tokio runtime)
- ✅ **YUYV to RGB conversion** - Manual color space conversion
- ✅ **MJPEG decoding** - Using image crate
- ✅ **FPS counter with smoothing** - Real-time performance metrics
- ✅ Frame statistics (frames received, frame rate)
- ✅ UI controls (Connect → Start → Stop → Disconnect)
- ✅ Texture management with GPU upload
- ✅ Proper cleanup on exit

### Other Crates
- 🚧 `crates/codec/` - **NEXT: Starting H.264 encoding implementation**
- ⬜ `crates/network/` - Not started (Phase 3)
- ⬜ `crates/crypto/` - Not started (Phase 4)

## Current Architecture

### Working Pipeline
```
Camera (V4L2) → CameraActor → Event Channel → Async Task → RGB Decode → GPU Texture → Display
```

### Next: Add Encoding
```
Camera → CameraActor → Frame → H.264 Encoder → MP4 File → VLC Playback
                         ↓
                     Display (parallel)
```

## Recent Session (2025-11-26)

### What We Discussed

1. **Reviewed Current Progress**
   - Discovered uncommitted changes (actor pattern, UI, race condition fix)
   - Fixed clippy warnings in capture crate
   - Code is ready to commit

2. **Chose Next Goal: H.264 Encoding (Option 2)**
   - Start with software encoder (x264enc) on laptop
   - Design for future hardware encoder support (Raspberry Pi)
   - Save encoded output to file for verification

3. **Created Comprehensive Implementation Plan**
   - **Plan Location**: `/home/erdem/.claude/plans/sequential-skipping-floyd.md`
   - Educational approach: Deep dive on GStreamer encoding pipeline
   - Step-by-step implementation guide
   - User will write code with my guidance (not me writing it)

### Plan Summary

**Goal**: Add H.264 encoding to `crates/codec/` using GStreamer

**Approach**:
- Use GStreamer pipeline: `appsrc → videoconvert → x264enc → mp4mux → filesink`
- Trait-based API for multiple encoder backends
- Start with software encoder (x264enc)
- Future: Hardware encoder detection (v4l2h264enc, vaapih264enc)

**Implementation Phases**:
1. ✅ Planning complete
2. ⏳ **Next**: Install GStreamer system dependencies
3. Implement core encoder (7 files)
4. Create integration tests
5. Integrate with UI (add recording button)

**Learning Focus**:
- GStreamer core concepts (Elements, Pads, Buffers, Caps, Pipelines)
- Deep dive on encoding pipeline architecture
- Caps negotiation and state management
- Frame to GstBuffer conversion with timestamps
- Error handling and debugging

## Next Steps (Priority Order)

### Immediate: Phase 2 - H.264 Encoding Setup
1. **Install GStreamer system dependencies**
   ```bash
   sudo apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev
   sudo apt install gstreamer1.0-plugins-{base,good,ugly,bad}
   ```

2. **Verify GStreamer installation**
   ```bash
   gst-inspect-1.0 x264enc
   gst-launch-1.0 videotestsrc num-buffers=10 ! x264enc ! mp4mux ! filesink location=test.mp4
   ```

3. **Add GStreamer Rust dependencies** to `crates/codec/Cargo.toml`
   - gstreamer = "0.23"
   - gstreamer-app = "0.23"
   - gstreamer-video = "0.23"

### Next: Core Implementation (7 Files)
4. Create `codec/src/lib.rs` - Public API and init()
5. Create `codec/src/error.rs` - CodecError enum
6. Create `codec/src/config.rs` - H264Config, EncoderPreset, H264Profile
7. Create `codec/src/buffer.rs` - Frame → GstBuffer conversion (timestamps!)
8. Create `codec/src/pipeline.rs` - GStreamer pipeline construction
9. Create `codec/src/encoder.rs` - GStreamerH264Encoder implementation
10. Create `codec/src/stats.rs` - EncoderStats

### Then: Testing & Integration
11. Create `codec/tests/integration_test.rs` - Encode synthetic frames
12. Run test and verify output with VLC
13. Add codec dependency to UI
14. Add recording button to UI
15. Test recording real camera footage

### Future: After Basic Encoding Works
16. Hardware encoder detection (v4l2h264enc for RPi)
17. Use appsink for network streaming (instead of filesink)
18. Performance metrics (encoding latency)
19. Adaptive bitrate control
20. Network transport (Phase 3)

## Current Blockers

**None** - Ready to start Phase 2 implementation

## Git Status

### Uncommitted Changes (Ready to Commit)
```
Modified:
  crates/capture/src/lib.rs  (actor pattern, clippy fixes)
  crates/core/src/lib.rs     (core types)
  crates/ui/src/main.rs      (full UI implementation)

Untracked:
  ARCHITECTURE_DECISIONS.md
  CLAUDE.md
  CURRENT_STATE.md
  WORKING_WITH_CLAUDE.md
```

**Recent Commits**:
- `c4ded2b` - Adds basic ui to get the frame streaming
- `4102745` - Fixs the race conditon when stopping the stream
- `fccbd2b` - Adds actor pattern to the camera control
- `4c6e75b` - Developing camera functionality
- `0ad10ab` - Adding camera capability discovery process

**Suggested Next Commit Message**:
```
Implement complete actor-based camera streaming with UI

Major additions:
- Actor pattern for camera control with command/event messaging
- Full CameraHandle API with lifecycle management
- egui-based video viewer with real-time frame display
- Frame decoding pipeline (MJPEG and YUYV to RGB)
- Background async frame processing
- FPS counter and statistics
- Race condition fix: update state before camera.stop()
- Comprehensive error handling and graceful shutdown

This completes Phase 1 basic streaming functionality.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
```

## Architecture Decisions Made

### 1. Actor Pattern for Camera Control ✅
- **Decision**: Use actor pattern with `std::thread` (not async)
- **Rationale**: `rscam` is blocking I/O, needs dedicated thread
- **Implementation**: CameraActor with mpsc channels (10 command, 100 event capacity)

### 2. Frame Flow Architecture ✅
- **Decision**: Separate decoding thread from UI thread
- **Implementation**:
  - Camera thread → Event channel → Tokio async task → Decodes to RGB → Arc<Mutex<>> → UI reads
  - UI never blocks on camera operations

### 3. UI Framework ✅
- **Decision**: egui with wgpu backend
- **Rationale**: Immediate mode, GPU-accelerated, simple integration

### 4. Next: Encoder Architecture 🚧
- **Decision**: Trait-based encoder abstraction (`VideoEncoder` trait)
- **Pipeline**: `appsrc → videoconvert → x264enc → mp4mux → filesink`
- **File format**: MP4 (widely compatible, easy to verify)
- **Timestamps**: Use frame_number arithmetic (not SystemTime)

## Performance Metrics (Current)

### Phase 1 Achievements
- ✅ Real-time 30fps video display
- ✅ FPS counter with exponential smoothing
- ✅ Frame sequence tracking
- ✅ No dropped frames at 640x480 YUYV
- ✅ Low latency (<100ms capture to display)

### Phase 2 Targets
- Encoding latency: <20ms per frame @ 640x480
- Real-time encoding at 30fps
- Output bitrate: ~2 Mbps (configurable)
- File size: ~7.5 MB per minute @ 2 Mbps

## Learning Journey

### Completed Concepts ✅
- Rust ownership and borrowing
- Actor pattern for concurrency
- Message passing with mpsc channels
- Thread management
- egui immediate mode UI
- Color space conversion (YUYV→RGB, MJPEG decoding)
- Async/await with Tokio

### Next: GStreamer Deep Dive 🚧
- Pipeline-based multimedia framework
- Elements, Pads, Buffers, Caps
- Caps negotiation
- State machine (NULL → READY → PAUSED → PLAYING)
- Timestamp management (PTS, duration)
- Error handling via bus messages
- Hardware encoder detection

## Resources

### Documentation
- **Implementation Plan**: `/home/erdem/.claude/plans/sequential-skipping-floyd.md`
- **Project Guide**: `CLAUDE.md`
- **Architecture Decisions**: `ARCHITECTURE_DECISIONS.md`

### Key Files to Work With (Phase 2)
1. `crates/codec/Cargo.toml` - Dependencies
2. `crates/codec/src/lib.rs` - Public API
3. `crates/codec/src/pipeline.rs` - GStreamer pipeline
4. `crates/codec/src/buffer.rs` - Timestamp calculation
5. `crates/codec/src/encoder.rs` - Main implementation

---

**Status**: Phase 1 Complete ✅ | Phase 2 Ready to Start 🚀

**Next Action**: Install GStreamer system dependencies and verify installation

**Last Session**: 2025-11-26 - Created H.264 encoding implementation plan, fixed clippy warnings, ready to commit Phase 1 work

---

*This file serves as memory for Claude Code between sessions. Update after each major milestone.*
