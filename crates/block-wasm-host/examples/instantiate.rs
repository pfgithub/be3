fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: instantiate PLUGIN.wasm");
        std::process::exit(2);
    };
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    let mut plugin = match block_wasm_host::Plugin::from_file(path.as_ref(), device, queue) {
        Ok(plugin) => plugin,
        Err(error) => {
            eprintln!("load failed: {error}");
            std::process::exit(1);
        }
    };
    println!("instantiated");
    if let Err(error) = plugin.start() {
        eprintln!("start failed: {error}");
        std::process::exit(1);
    }
    let outbound = plugin.take_outbound();
    println!("start sent {} frame(s)", outbound.len());
    for frame in &outbound {
        match block_plugin_api::decode_frame(frame) {
            Ok(message) => println!("  {message:?}"),
            Err(error) => println!("  undecodable: {error:?}"),
        }
    }
}
