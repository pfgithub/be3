use block_plugin_api::{
    decode_frame, encode_frame, EditorInstanceId, EditorMessage, EditorRegion, HelloAccepted,
    Message, ScreenId, ScreenRequest, ScreenSet, ViewportMetrics, PROTOCOL_VERSION,
};
use block_wasm_host::Plugin;

const SCREENS_SURFACE: u32 = 0;
const WIDTH: u32 = 640;
const HEIGHT: u32 = 400;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: instantiate PLUGIN.wasm");
        std::process::exit(2);
    };
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    let mut plugin = match Plugin::from_file(path.as_ref(), device, queue) {
        Ok(plugin) => plugin,
        Err(error) => {
            eprintln!("load failed: {error}");
            std::process::exit(1);
        }
    };
    println!("instantiated");
    check(plugin.start());
    report("start", &mut plugin);
    send(&mut plugin, hello_accepted());
    send(&mut plugin, open());
    send(&mut plugin, screens());
    check(plugin.step());
    report("open", &mut plugin);
    send(&mut plugin, Message::DrawFrame);
    check(plugin.step());
    let drawn = report("draw", &mut plugin);
    match plugin.surface(SCREENS_SURFACE) {
        Some((texture, generation)) => println!(
            "surface {SCREENS_SURFACE} is {}x{} at generation {generation}",
            texture.width(),
            texture.height()
        ),
        None => {
            eprintln!("the plugin never asked for a surface");
            std::process::exit(1);
        }
    }
    if !drawn {
        eprintln!("the plugin never reported a frame");
        std::process::exit(1);
    }
    if !plugin.take_presented().contains(&SCREENS_SURFACE) {
        eprintln!("the plugin never presented its surface");
        std::process::exit(1);
    }
    println!("presented");
    plugin.stop();
}

fn report(stage: &str, plugin: &mut Plugin) -> bool {
    let outbound = plugin.take_outbound();
    println!("{stage} sent {} frame(s)", outbound.len());
    let mut drawn = false;
    for frame in &outbound {
        match decode_frame(frame) {
            Ok(message) => {
                drawn |= matches!(message, Message::FrameReady(_));
                println!("  {message:?}");
            }
            Err(error) => println!("  undecodable: {error:?}"),
        }
    }
    drawn
}

fn send(plugin: &mut Plugin, message: Message) {
    match encode_frame(&message) {
        Ok(frame) => plugin.send(frame),
        Err(error) => {
            eprintln!("could not encode {message:?}: {error}");
            std::process::exit(1);
        }
    }
}

fn check(outcome: Result<(), String>) {
    if let Err(error) = outcome {
        eprintln!("the plugin failed: {error}");
        std::process::exit(1);
    }
}

fn hello_accepted() -> Message {
    Message::HelloAccepted(HelloAccepted {
        version: PROTOCOL_VERSION,
        host_name: "instantiate".to_owned(),
        capabilities: Vec::new(),
        dark_theme: true,
    })
}

fn open() -> Message {
    Message::Editor(EditorMessage::Open {
        instance: EditorInstanceId(1),
        block_id: [1; 16],
        block_type: [2; 16],
        account_id: [3; 16],
        workspace_id: [4; 16],
        editable: true,
    })
}

fn screens() -> Message {
    Message::Screens(ScreenSet {
        request_id: 1,
        screens: vec![ScreenRequest {
            screen: ScreenId(1),
            instance: EditorInstanceId(1),
            region: EditorRegion::Main,
            metrics: ViewportMetrics {
                logical_width: WIDTH as f32,
                logical_height: HEIGHT as f32,
                visible_x: 0.0,
                visible_y: 0.0,
                pixel_width: WIDTH,
                pixel_height: HEIGHT,
                scale_factor: 1.0,
            },
        }],
    })
}
