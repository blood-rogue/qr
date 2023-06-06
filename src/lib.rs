pub mod encode;
mod utils;

pub fn get_pixel_data(data: String) -> utils::Pixels {
    encode::encode(data)
}

pub fn generate_png(data: String, filename: String) {
    encode::to_png(encode::encode(data), filename)
}