# Implementation Phases - Step-by-Step Guide

## Phase 1: Foundation (Week 1-2)

### Step 1.1: Project Setup

**Create Workspace Structure:**
```bash
mkdir rust-secure-streaming && cd rust-secure-streaming
cargo init --name streaming-workspace

# Create workspace structure
mkdir -p crates/{core,capture,codec,network,crypto,ui}
```

**Workspace Cargo.toml:**
```toml
[workspace]
members = [
    "crates/core",
    "crates/capture", 
    "crates/codec",
    "crates/network",
    "crates/crypto",
    "crates/ui",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["Your Name <your.email@example.com>"]
license = "MIT OR Apache-2.0"
repository = "https://github.com/yourusername/rust-secure-streaming"

[workspace.dependencies]
tokio = { version = "1.35", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1.0"
thiserror = "1.0"
serde = { version = "1.0", features = ["derive"] }
bytes = "1.5"
```

### Step 1.2: Core Abstractions

**crates/core/src/lib.rs:**
```rust
use bytes::Bytes;
use std::time::{Duration, SystemTime};

/// Core frame type used throughout the pipeline
#[derive(Debug, Clone)]
pub struct Frame {
    /// Frame data in native format (YUV, RGB, etc)
    pub data: Bytes,
    /// Pixel format
    pub format: PixelFormat,
    /// Frame dimensions
    pub width: u32,
    pub height: u32,
    /// Capture timestamp
    pub timestamp: SystemTime,
    /// Sequence number for ordering
    pub sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PixelFormat {
    YUYV,
    NV12,
    I420,
    RGB24,
    BGR24,
    MJPEG,
    H264,
}

/// Video source trait - all cameras must implement this
#[async_trait::async_trait]
pub trait VideoSource: Send + Sync {
    /// Capture a single frame
    async fn capture_frame(&mut self) -> Result<Frame, CaptureError>;
    
    /// Get camera capabilities
    fn capabilities(&self) -> &CameraCapabilities;
    
    /// Configure camera settings
    async fn configure(&mut self, config: CaptureConfig) -> Result<(), CaptureError>;
    
    /// Start continuous capture
    async fn start(&mut self) -> Result<(), CaptureError>;
    
    /// Stop capture
    async fn stop(&mut self) -> Result<(), CaptureError>;
}

#[derive(Debug, Clone)]
pub struct CameraCapabilities {
    pub formats: Vec<PixelFormat>,
    pub resolutions: Vec<(u32, u32)>,
    pub frame_rates: Vec<u32>,
    pub hardware_encoding: bool,
}

#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub format: PixelFormat,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("Device not found: {0}")]
    DeviceNotFound(String),
    #[error("Unsupported format: {0:?}")]
    UnsupportedFormat(PixelFormat),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
```

### Step 1.3: Basic Camera Capture

**crates/capture/Cargo.toml:**
```toml
[package]
name = "streaming-capture"
version.workspace = true
edition.workspace = true

[dependencies]
streaming-core = { path = "../core" }
tokio.workspace = true
tracing.workspace = true
anyhow.workspace = true
bytes.workspace = true
async-trait = "0.1"

# Linux V4L2 support
[target.'cfg(target_os = "linux")'.dependencies]
v4l = "0.14"
nix = { version = "0.27", features = ["ioctl"] }
memmap2 = "0.9"
```

**crates/capture/src/v4l2.rs:**
```rust
use streaming_core::{Frame, VideoSource, CameraCapabilities, CaptureConfig, CaptureError, PixelFormat};
use v4l::prelude::*;
use v4l::video::Capture;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct V4L2Camera {
    device: Arc<Mutex<Device>>,
    capabilities: CameraCapabilities,
    config: CaptureConfig,
    sequence: u64,
}

impl V4L2Camera {
    pub fn new(device_path: &str) -> Result<Self, CaptureError> {
        let device = Device::with_path(device_path)
            .map_err(|e| CaptureError::DeviceNotFound(e.to_string()))?;
        
        // Query capabilities
        let caps = device.query_caps()
            .map_err(|e| CaptureError::Io(std::io::Error::new(
                std::io::ErrorKind::Other, e
            )))?;
        
        // Build capabilities struct
        let capabilities = CameraCapabilities {
            formats: vec![PixelFormat::YUYV, PixelFormat::MJPEG],
            resolutions: vec![(640, 480), (1280, 720), (1920, 1080)],
            frame_rates: vec![30, 60],
            hardware_encoding: caps.capabilities.contains(Flags::VIDEO_M2M),
        };
        
        let config = CaptureConfig {
            format: PixelFormat::YUYV,
            width: 640,
            height: 480,
            fps: 30,
        };
        
        Ok(Self {
            device: Arc::new(Mutex::new(device)),
            capabilities,
            config,
            sequence: 0,
        })
    }
}

#[async_trait::async_trait]
impl VideoSource for V4L2Camera {
    async fn capture_frame(&mut self) -> Result<Frame, CaptureError> {
        let mut device = self.device.lock().await;
        
        // Use MMAP for zero-copy capture
        let frame = device
            .capture_frame()
            .map_err(|e| CaptureError::Io(std::io::Error::new(
                std::io::ErrorKind::Other, e
            )))?;
        
        self.sequence += 1;
        
        Ok(Frame {
            data: Bytes::copy_from_slice(&frame.data),
            format: self.config.format,
            width: self.config.width,
            height: self.config.height,
            timestamp: std::time::SystemTime::now(),
            sequence: self.sequence,
        })
    }
    
    fn capabilities(&self) -> &CameraCapabilities {
        &self.capabilities
    }
    
    async fn configure(&mut self, config: CaptureConfig) -> Result<(), CaptureError> {
        let mut device = self.device.lock().await;
        
        // Set format
        let format = Format::new(config.width, config.height, FourCC::new(b"YUYV"));
        device.set_format(&format)
            .map_err(|e| CaptureError::Io(std::io::Error::new(
                std::io::ErrorKind::Other, e
            )))?;
        
        self.config = config;
        Ok(())
    }
    
    async fn start(&mut self) -> Result<(), CaptureError> {
        let mut device = self.device.lock().await;
        device.start_stream()
            .map_err(|e| CaptureError::Io(std::io::Error::new(
                std::io::ErrorKind::Other, e
            )))?;
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<(), CaptureError> {
        let mut device = self.device.lock().await;
        device.stop_stream()
            .map_err(|e| CaptureError::Io(std::io::Error::new(
                std::io::ErrorKind::Other, e
            )))?;
        Ok(())
    }
}
```

## Phase 2: GStreamer Integration (Week 3-4)

### Step 2.1: GStreamer Pipeline Setup

**crates/codec/Cargo.toml:**
```toml
[dependencies]
streaming-core = { path = "../core" }
gstreamer = { version = "0.21", features = ["v1_22"] }
gstreamer-app = "0.21"
gstreamer-video = "0.21"
tokio.workspace = true
tracing.workspace = true
bytes.workspace = true
```

**crates/codec/src/encoder.rs:**
```rust
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app::{AppSink, AppSrc};
use streaming_core::{Frame, PixelFormat};
use bytes::Bytes;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

pub struct GStreamerEncoder {
    pipeline: gst::Pipeline,
    appsrc: AppSrc,
    appsink: AppSink,
    encoded_tx: mpsc::Sender<Bytes>,
}

impl GStreamerEncoder {
    pub fn new_hardware_accelerated() -> Result<Self, gst::Error> {
        gst::init()?;
        
        // Detect hardware encoder
        let encoder = Self::detect_hardware_encoder();
        
        // Build pipeline string
        let pipeline_str = format!(
            "appsrc name=src ! \
             videoconvert ! \
             video/x-raw,format=I420 ! \
             {} bitrate=4000000 tune=zerolatency ! \
             h264parse ! \
             appsink name=sink",
            encoder
        );
        
        let pipeline = gst::parse_launch(&pipeline_str)?
            .downcast::<gst::Pipeline>()
            .expect("Expected a pipeline");
        
        let appsrc = pipeline
            .by_name("src")
            .expect("Source not found")
            .downcast::<AppSrc>()
            .expect("Not an AppSrc");
        
        let appsink = pipeline
            .by_name("sink")
            .expect("Sink not found")
            .downcast::<AppSink>()
            .expect("Not an AppSink");
        
        // Configure appsrc
        appsrc.set_caps(Some(&gst::Caps::builder("video/x-raw")
            .field("format", "YUY2")
            .field("width", 1920i32)
            .field("height", 1080i32)
            .field("framerate", gst::Fraction::new(30, 1))
            .build()));
        
        appsrc.set_format(gst::Format::Time);
        appsrc.set_block(false);
        
        // Configure appsink
        appsink.set_caps(Some(&gst::Caps::builder("video/x-h264")
            .field("stream-format", "byte-stream")
            .build()));
        
        let (encoded_tx, mut encoded_rx) = mpsc::channel::<Bytes>(30);
        
        // Setup appsink callback
        let tx_clone = encoded_tx.clone();
        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let sample = sink.pull_sample().map_err(|_| gst::FlowError::Error)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                    let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
                    
                    let data = Bytes::copy_from_slice(map.as_slice());
                    tx_clone.blocking_send(data).map_err(|_| gst::FlowError::Error)?;
                    
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
        
        Ok(Self {
            pipeline,
            appsrc,
            appsink,
            encoded_tx,
        })
    }
    
    fn detect_hardware_encoder() -> &'static str {
        // Check for hardware encoders in order of preference
        let encoders = [
            ("v4l2h264enc", "V4L2 hardware encoder"),     // Raspberry Pi
            ("nvv4l2h264enc", "NVIDIA hardware encoder"), // Jetson
            ("vaapih264enc", "Intel VAAPI encoder"),      // Intel
            ("omxh264enc", "OpenMAX encoder"),            // Older RPi
            ("x264enc", "Software encoder"),              // Fallback
        ];
        
        for (encoder, description) in &encoders {
            if gst::ElementFactory::find(encoder).is_some() {
                tracing::info!("Using {}: {}", encoder, description);
                return encoder;
            }
        }
        
        "x264enc" // Default fallback
    }
    
    pub async fn encode_frame(&mut self, frame: Frame) -> Result<(), gst::Error> {
        // Create GStreamer buffer from frame
        let mut buffer = gst::Buffer::with_size(frame.data.len()).unwrap();
        {
            let buffer_ref = buffer.get_mut().unwrap();
            let mut map = buffer_ref.map_writable().unwrap();
            map.as_mut_slice().copy_from_slice(&frame.data);
        }
        
        // Push to pipeline
        self.appsrc.push_buffer(buffer).map_err(|_| gst::Error::Failed)?;
        
        Ok(())
    }
    
    pub async fn start(&self) -> Result<(), gst::Error> {
        self.pipeline.set_state(gst::State::Playing)?;
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<(), gst::Error> {
        self.pipeline.set_state(gst::State::Null)?;
        Ok(())
    }
    
    pub fn get_encoded_receiver(&self) -> mpsc::Receiver<Bytes> {
        // This would need proper implementation
        unimplemented!("Return the receiver for encoded frames")
    }
}
```

## Phase 3: WebRTC Setup (Week 5-6)

### Step 3.1: WebRTC Signaling and Connection

**crates/network/Cargo.toml:**
```toml
[dependencies]
streaming-core = { path = "../core" }
webrtc = "0.9"
tokio.workspace = true
tracing.workspace = true
serde.workspace = true
serde_json = "1.0"
anyhow.workspace = true
```

**crates/network/src/webrtc_peer.rs:**
```rust
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::TrackLocal;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct WebRTCPeer {
    peer_connection: Arc<RTCPeerConnection>,
    video_track: Arc<TrackLocalStaticRTP>,
}

impl WebRTCPeer {
    pub async fn new(is_offerer: bool) -> Result<Self, webrtc::Error> {
        // Create API with required settings
        let api = APIBuilder::new().build();
        
        // Configure ICE servers
        let config = RTCConfiguration {
            ice_servers: vec![
                RTCIceServer {
                    urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        
        // Create peer connection
        let peer_connection = Arc::new(api.new_peer_connection(config).await?);
        
        // Create video track
        let video_track = Arc::new(TrackLocalStaticRTP::new(
            RTCRtpCodecCapability {
                mime_type: "video/H264".to_owned(),
                clock_rate: 90000,
                channels: 0,
                sdp_fmtp_line: "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f".to_owned(),
                rtcp_feedback: vec![],
            },
            "video".to_owned(),
            "webrtc-rs".to_owned(),
        ));
        
        // Add track to peer connection
        let rtp_sender = peer_connection
            .add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
            .await?;
        
        // Set up state change handler
        let pc_clone = Arc::clone(&peer_connection);
        peer_connection.on_peer_connection_state_change(Box::new(move |state| {
            tracing::info!("Peer connection state: {:?}", state);
            Box::pin(async {})
        }));
        
        // Set up ICE candidate handler
        peer_connection.on_ice_candidate(Box::new(move |candidate| {
            if let Some(candidate) = candidate {
                tracing::info!("New ICE candidate: {}", candidate.candidate);
            }
            Box::pin(async {})
        }));
        
        Ok(Self {
            peer_connection,
            video_track,
        })
    }
    
    pub async fn create_offer(&self) -> Result<String, webrtc::Error> {
        let offer = self.peer_connection.create_offer(None).await?;
        self.peer_connection.set_local_description(offer.clone()).await?;
        
        // Serialize offer to JSON for signaling
        Ok(serde_json::to_string(&offer).unwrap())
    }
    
    pub async fn create_answer(&self, offer_sdp: String) -> Result<String, webrtc::Error> {
        // Parse offer
        let offer: webrtc::peer_connection::sdp::session_description::RTCSessionDescription = 
            serde_json::from_str(&offer_sdp).unwrap();
        
        self.peer_connection.set_remote_description(offer).await?;
        
        let answer = self.peer_connection.create_answer(None).await?;
        self.peer_connection.set_local_description(answer.clone()).await?;
        
        Ok(serde_json::to_string(&answer).unwrap())
    }
    
    pub async fn set_remote_answer(&self, answer_sdp: String) -> Result<(), webrtc::Error> {
        let answer: webrtc::peer_connection::sdp::session_description::RTCSessionDescription = 
            serde_json::from_str(&answer_sdp).unwrap();
        
        self.peer_connection.set_remote_description(answer).await?;
        Ok(())
    }
    
    pub async fn send_video_frame(&self, data: &[u8]) -> Result<(), webrtc::Error> {
        self.video_track.write_rtp(&rtp::packet::Packet {
            header: rtp::header::Header {
                version: 2,
                padding: false,
                extension: false,
                marker: true,
                payload_type: 96,
                sequence_number: 0, // Should increment
                timestamp: 0,       // Should be proper timestamp
                ssrc: 0,
                ..Default::default()
            },
            payload: data.to_vec(),
        }).await?;
        
        Ok(())
    }
}
```

## Phase 4: Security Implementation (Week 7-8)

### Step 4.1: TLS with rustls

**crates/crypto/Cargo.toml:**
```toml
[dependencies]
rustls = { version = "0.22", features = ["dangerous_configuration"] }
rustls-pemfile = "2.0"
tokio-rustls = "0.25"
ring = "0.17"
rcgen = "0.12"  # For certificate generation
x509-parser = "0.15"
webpki = "0.22"
```

**crates/crypto/src/tls_config.rs:**
```rust
use rustls::{ClientConfig, ServerConfig, Certificate, PrivateKey};
use rustls::internal::msgs::persist;
use std::io::BufReader;
use std::fs::File;
use std::path::Path;
use rcgen::{CertificateParams, DistinguishedName, KeyPair};

pub struct TLSManager {
    root_cert: Certificate,
    server_config: Arc<ServerConfig>,
    client_config: Arc<ClientConfig>,
}

impl TLSManager {
    /// Generate a new self-signed certificate authority
    pub fn generate_ca() -> Result<(Certificate, PrivateKey), Box<dyn std::error::Error>> {
        let mut params = CertificateParams::default();
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(
            rcgen::DnType::CountryName,
            "US"
        );
        params.distinguished_name.push(
            rcgen::DnType::OrganizationName,
            "Secure Streaming CA"
        );
        params.distinguished_name.push(
            rcgen::DnType::CommonName,
            "Root CA"
        );
        
        let ca_cert = rcgen::Certificate::from_params(params)?;
        let ca_cert_pem = ca_cert.serialize_pem()?;
        let ca_key_pem = ca_cert.serialize_private_key_pem();
        
        let cert = Certificate(ca_cert_pem.into_bytes());
        let key = PrivateKey(ca_key_pem.into_bytes());
        
        Ok((cert, key))
    }
    
    /// Generate a device certificate signed by CA
    pub fn generate_device_cert(
        ca_cert: &rcgen::Certificate,
        device_id: &str,
    ) -> Result<(Certificate, PrivateKey), Box<dyn std::error::Error>> {
        let mut params = CertificateParams::default();
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(
            rcgen::DnType::CommonName,
            device_id
        );
        params.subject_alt_names = vec![
            rcgen::SanType::DnsName(device_id.to_string()),
        ];
        
        let device_cert = rcgen::Certificate::from_params(params)?;
        let device_cert_pem = device_cert.serialize_pem_with_signer(ca_cert)?;
        let device_key_pem = device_cert.serialize_private_key_pem();
        
        let cert = Certificate(device_cert_pem.into_bytes());
        let key = PrivateKey(device_key_pem.into_bytes());
        
        Ok((cert, key))
    }
    
    /// Create server configuration with mutual TLS
    pub fn create_server_config(
        cert_chain: Vec<Certificate>,
        private_key: PrivateKey,
        client_ca: Certificate,
    ) -> Result<ServerConfig, rustls::Error> {
        let mut root_store = rustls::RootCertStore::empty();
        root_store.add(&client_ca)?;
        
        let client_cert_verifier = rustls::server::AllowAnyAuthenticatedClient::new(root_store);
        
        let config = ServerConfig::builder()
            .with_safe_default_cipher_suites()
            .with_safe_default_kx_groups()
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_client_cert_verifier(Arc::new(client_cert_verifier))
            .with_single_cert(cert_chain, private_key)?;
        
        Ok(config)
    }
    
    /// Create client configuration with mutual TLS
    pub fn create_client_config(
        cert_chain: Vec<Certificate>,
        private_key: PrivateKey,
        server_ca: Certificate,
    ) -> Result<ClientConfig, rustls::Error> {
        let mut root_store = rustls::RootCertStore::empty();
        root_store.add(&server_ca)?;
        
        let config = ClientConfig::builder()
            .with_safe_default_cipher_suites()
            .with_safe_default_kx_groups()
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_root_certificates(root_store)
            .with_client_auth_cert(cert_chain, private_key)?;
        
        Ok(config)
    }
}

/// Certificate pinning implementation
pub struct CertificatePinner {
    pinned_hashes: Vec<[u8; 32]>,  // SHA-256 hashes
}

impl CertificatePinner {
    pub fn new() -> Self {
        Self {
            pinned_hashes: Vec::new(),
        }
    }
    
    pub fn add_pin(&mut self, cert: &Certificate) {
        use ring::digest;
        let hash = digest::digest(&digest::SHA256, &cert.0);
        let mut pin = [0u8; 32];
        pin.copy_from_slice(hash.as_ref());
        self.pinned_hashes.push(pin);
    }
    
    pub fn verify(&self, cert: &Certificate) -> bool {
        use ring::digest;
        let hash = digest::digest(&digest::SHA256, &cert.0);
        let mut pin = [0u8; 32];
        pin.copy_from_slice(hash.as_ref());
        
        self.pinned_hashes.contains(&pin)
    }
}
```

## Phase 5: UI Implementation (Week 9-10)

### Step 5.1: egui Video Player

**crates/ui/Cargo.toml:**
```toml
[dependencies]
eframe = { version = "0.25", features = ["wgpu"] }
egui = "0.25"
image = { version = "0.24", features = ["jpeg", "png"] }
tokio = { workspace = true, features = ["rt-multi-thread"] }
streaming-core = { path = "../core" }
crossbeam-channel = "0.5"
```

**crates/ui/src/video_player.rs:**
```rust
use eframe::egui;
use egui::{ColorImage, TextureHandle, TextureOptions};
use streaming_core::Frame;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct VideoPlayer {
    texture: Option<TextureHandle>,
    current_frame: Arc<Mutex<Option<Frame>>>,
    stats: StreamingStats,
}

#[derive(Default)]
pub struct StreamingStats {
    pub fps: f32,
    pub bitrate: f32,
    pub latency_ms: f32,
    pub frames_received: u64,
    pub frames_dropped: u64,
}

impl VideoPlayer {
    pub fn new() -> Self {
        Self {
            texture: None,
            current_frame: Arc::new(Mutex::new(None)),
            stats: StreamingStats::default(),
        }
    }
    
    pub async fn update_frame(&mut self, frame: Frame) {
        let mut current = self.current_frame.lock().await;
        *current = Some(frame);
        self.stats.frames_received += 1;
    }
    
    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Main video display
        ui.group(|ui| {
            ui.heading("Live Stream");
            
            // Convert frame to texture if available
            if let Ok(frame_opt) = self.current_frame.try_lock() {
                if let Some(frame) = frame_opt.as_ref() {
                    // Convert YUV to RGB (simplified)
                    let rgb_data = self.yuv_to_rgb(frame);
                    
                    let color_image = ColorImage::from_rgb(
                        [frame.width as usize, frame.height as usize],
                        &rgb_data,
                    );
                    
                    // Create or update texture
                    let texture = self.texture.get_or_insert_with(|| {
                        ctx.load_texture(
                            "video_frame",
                            color_image.clone(),
                            TextureOptions::LINEAR,
                        )
                    });
                    
                    texture.set(color_image, TextureOptions::LINEAR);
                    
                    // Display the texture
                    ui.image(texture);
                }
            }
            
            // Statistics overlay
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(format!("FPS: {:.1}", self.stats.fps));
                ui.separator();
                ui.label(format!("Bitrate: {:.1} Mbps", self.stats.bitrate / 1_000_000.0));
                ui.separator();
                ui.label(format!("Latency: {:.1} ms", self.stats.latency_ms));
                ui.separator();
                ui.label(format!("Frames: {} / Dropped: {}", 
                    self.stats.frames_received, 
                    self.stats.frames_dropped
                ));
            });
        });
    }
    
    fn yuv_to_rgb(&self, frame: &Frame) -> Vec<u8> {
        // Simplified YUV to RGB conversion
        // In production, use a proper color space converter or GPU shader
        let mut rgb = Vec::with_capacity((frame.width * frame.height * 3) as usize);
        
        // This is a placeholder - implement proper conversion based on format
        for i in 0..frame.data.len() {
            rgb.push(frame.data[i]);
            if rgb.len() >= (frame.width * frame.height * 3) as usize {
                break;
            }
        }
        
        rgb
    }
}
```

**crates/ui/src/main.rs:**
```rust
use eframe::egui;
use streaming_ui::video_player::VideoPlayer;
use std::sync::Arc;
use tokio::runtime::Runtime;

struct StreamingApp {
    video_player: VideoPlayer,
    runtime: Arc<Runtime>,
    connection_status: ConnectionStatus,
}

#[derive(Debug, Clone, PartialEq)]
enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

impl Default for StreamingApp {
    fn default() -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
        );
        
        Self {
            video_player: VideoPlayer::new(),
            runtime,
            connection_status: ConnectionStatus::Disconnected,
        }
    }
}

impl eframe::App for StreamingApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top panel with controls
        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Secure Video Streaming");
                
                ui.separator();
                
                // Connection controls
                match &self.connection_status {
                    ConnectionStatus::Disconnected => {
                        if ui.button("Connect").clicked() {
                            self.connect();
                        }
                    }
                    ConnectionStatus::Connecting => {
                        ui.spinner();
                        ui.label("Connecting...");
                    }
                    ConnectionStatus::Connected => {
                        ui.colored_label(egui::Color32::GREEN, "● Connected");
                        if ui.button("Disconnect").clicked() {
                            self.disconnect();
                        }
                    }
                    ConnectionStatus::Error(err) => {
                        ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
                        if ui.button("Retry").clicked() {
                            self.connect();
                        }
                    }
                }
                
                ui.separator();
                
                // Settings button
                if ui.button("⚙ Settings").clicked() {
                    // Open settings window
                }
            });
        });
        
        // Central panel with video
        egui::CentralPanel::default().show(ctx, |ui| {
            self.video_player.ui(ui, ctx);
        });
        
        // Request repaint for video updates
        ctx.request_repaint_after(std::time::Duration::from_millis(33)); // ~30fps
    }
}

impl StreamingApp {
    fn connect(&mut self) {
        self.connection_status = ConnectionStatus::Connecting;
        
        let runtime = Arc::clone(&self.runtime);
        runtime.spawn(async move {
            // Implement connection logic
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            // Update status through a channel
        });
    }
    
    fn disconnect(&mut self) {
        self.connection_status = ConnectionStatus::Disconnected;
        // Implement disconnection logic
    }
}

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([800.0, 600.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    
    eframe::run_native(
        "Secure Video Streaming",
        options,
        Box::new(|_cc| Box::new(StreamingApp::default())),
    )
}
```

## Phase 6: Integration & Testing (Week 11-12)

### Step 6.1: Complete Integration Test

**tests/integration_test.rs:**
```rust
use streaming_core::{Frame, VideoSource};
use streaming_capture::V4L2Camera;
use streaming_codec::GStreamerEncoder;
use streaming_network::WebRTCPeer;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_end_to_end_streaming() {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Setup camera
    let mut camera = V4L2Camera::new("/dev/video0")
        .expect("Failed to open camera");
    
    // Setup encoder
    let mut encoder = GStreamerEncoder::new_hardware_accelerated()
        .expect("Failed to create encoder");
    
    // Setup WebRTC
    let peer = WebRTCPeer::new(true).await
        .expect("Failed to create WebRTC peer");
    
    // Start streaming pipeline
    camera.start().await.expect("Failed to start camera");
    encoder.start().await.expect("Failed to start encoder");
    
    // Capture and stream frames
    for _ in 0..30 {  // Stream for 1 second at 30fps
        let frame = camera.capture_frame().await
            .expect("Failed to capture frame");
        
        encoder.encode_frame(frame).await
            .expect("Failed to encode frame");
        
        sleep(Duration::from_millis(33)).await;
    }
    
    // Cleanup
    camera.stop().await.expect("Failed to stop camera");
    encoder.stop().await.expect("Failed to stop encoder");
}
```

### Step 6.2: Performance Benchmarks

**benches/streaming_bench.rs:**
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use streaming_codec::GStreamerEncoder;
use streaming_core::{Frame, PixelFormat};
use bytes::Bytes;

fn create_test_frame(width: u32, height: u32) -> Frame {
    let size = (width * height * 2) as usize;  // YUV422
    Frame {
        data: Bytes::from(vec![0u8; size]),
        format: PixelFormat::YUYV,
        width,
        height,
        timestamp: std::time::SystemTime::now(),
        sequence: 0,
    }
}

fn benchmark_encoding(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("h264_encoding");
    
    for (width, height) in &[(640, 480), (1280, 720), (1920, 1080)] {
        let frame = create_test_frame(*width, *height);
        
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}x{}", width, height)),
            &frame,
            |b, frame| {
                b.to_async(&rt).iter(|| async {
                    let mut encoder = GStreamerEncoder::new_hardware_accelerated()
                        .unwrap();
                    encoder.encode_frame(frame.clone()).await.unwrap();
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(benches, benchmark_encoding);
criterion_main!(benches);
```

## Deployment & CI/CD

### GitHub Actions Workflow

**.github/workflows/ci.yml:**
```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v3
    
    - name: Install system dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y \
          libgstreamer1.0-dev \
          libgstreamer-plugins-base1.0-dev \
          gstreamer1.0-plugins-good \
          gstreamer1.0-plugins-bad \
          libv4l-dev \
          pkg-config
    
    - name: Setup Rust
      uses: actions-rs/toolchain@v1
      with:
        profile: minimal
        toolchain: stable
        override: true
        components: rustfmt, clippy
    
    - name: Cache dependencies
      uses: actions/cache@v3
      with:
        path: |
          ~/.cargo/registry
          ~/.cargo/git
          target
        key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
    
    - name: Check formatting
      run: cargo fmt --all -- --check
    
    - name: Run clippy
      run: cargo clippy --all-targets --all-features -- -D warnings
    
    - name: Run tests
      run: cargo test --all-features
    
    - name: Run benchmarks (without comparing)
      run: cargo bench --no-run

  security:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v3
    - name: Security audit
      uses: actions-rs/audit-check@v1
      with:
        token: ${{ secrets.GITHUB_TOKEN }}
```

---

*This implementation guide provides a solid foundation. Each phase builds upon the previous, ensuring you develop strong Rust skills while creating a production-quality system.*
