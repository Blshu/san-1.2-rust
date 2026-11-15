mod app;
mod document;
mod drawing;
mod font;
mod graphics;
mod hangul;
mod macros;

fn main() {
    if let Err(error) = app::run() {
        eprintln!("SAN: {error}");
        std::process::exit(1);
    }
}
