mod encode;
mod consts;

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let pixels = encode::encode(args[1].clone());
    encode::to_png(pixels, args[2].clone());
}
