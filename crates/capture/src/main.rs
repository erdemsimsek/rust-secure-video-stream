use streaming_capture::{discover_cameras, CameraInstance};

fn main() {
    let cameras = discover_cameras();

    if cameras.is_empty() {
        println!("No camera devices found in /dev/");
        return;
    }

    println!("Found {} camera device(s)\n", cameras.len());

    let mut camera_instances: Vec<CameraInstance> = cameras
        .iter()
        .map(|name| CameraInstance::new(name.to_string()))
        .collect();

    // Discover capabilities for each camera
    for camera in &mut camera_instances {
        camera.discover_capabilities();
    }

    // Print all discovered capabilities
    println!("=== Camera Capabilities ===\n");
    for camera in &camera_instances {
        camera.print_capabilities();
        println!();
    }
}
