

use streaming_capture::{spawn_camera_actor, CameraCommand, CameraEvent};

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let camera_instace = "/dev/video0";

    if let Ok((handle, mut events)) = spawn_camera_actor(camera_instace) {
        handle.send_command(CameraCommand::DiscoverCapabilities).unwrap();

        // Event loop
        while let Some(event) = events.blocking_recv() {
            match event {
                CameraEvent::CapabilitiesDiscovered(caps) => {
                    println!("Capabilities discovered!");
                    // Pick first format and resolution
                    if let Some(format) = caps.formats.first() {
                        if let Some(res) = format.resolutions.first() {
                            println!("Configuring: {:?} {}x{}", format.format, res.width, res.height);
                            handle.send_command(CameraCommand::SetConfiguration {
                                width: res.width,
                                height: res.height,
                                fps: 30,
                                format: format.format,
                            })?;
                        }
                    }
                }
                CameraEvent::Configured => {
                    println!("Configured! Starting streaming...");
                    handle.send_command(CameraCommand::StartStreaming)?;
                }
                CameraEvent::StreamingStarted => {
                    println!("Streaming started!");
                }
                CameraEvent::FrameCaptured(frame) => {
                    println!("Frame {}: {}x{} ({} bytes)",
                             frame.sequence, frame.width, frame.height, frame.data.len());
                }
                CameraEvent::StreamingStopped => {
                    println!("Streaming stopped!");
                    break; // Exit loop
                }
                CameraEvent::Error(e) => {
                    eprintln!("Error: {}", e);
                    break;
                }
                _ => {}
            }
        }


    }

    Ok(())
}
