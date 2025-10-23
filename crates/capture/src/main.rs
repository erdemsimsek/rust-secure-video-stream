use streaming_capture::{discover_cameras, CameraInstance};
use streaming_core::PixelFormat;

fn main() {

    let mut camera = CameraInstance::new("/dev/video0".to_string());
    camera.discover_capabilities();
    camera.print_capabilities();
    camera.configure(1280, 720, 30, PixelFormat::MJPG).unwrap();
    for i in 0..10 {
        if let Ok(frame) = camera.capture_frame() {
            println!("Frame data: {:?}", frame.data);
        }
    }

}
