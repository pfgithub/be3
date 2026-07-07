mod apps;
mod renderer;
mod text;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    renderer::run()
}
