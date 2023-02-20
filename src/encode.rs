use image::{ImageBuffer, RgbImage, Rgb};
use reed_solomon::Encoder;

use crate::consts::*;

const PIXELS: usize = 37;
const MODE: [u8; 4] = [0, 0, 1, 0];

type Pixels = [[bool; PIXELS]; PIXELS];

fn pad_left(bits: Vec<u8>, bit: u8, len: usize) -> Vec<u8> {
    let mut out = vec![bit].repeat(len - bits.len());
    out.extend_from_slice(bits.as_slice());
    out
}

fn pad_right_mut(bits: &mut Vec<u8>, bit: u8, len: usize) {
    bits.extend_from_slice(vec![bit].repeat(len - bits.len()).as_slice());
}

const MASKS: [u16; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768];

fn u8_to_bits(inp: u8) -> [u8; 8] {
    let mut out = [0; 8];
    for i in 0..8 {
        out[7-i] = (inp & MASKS[i] as u8) >> i
    }

    out
}

fn bits_to_u8(bits: &[u8; 8]) -> u8 {
    let mut out = 0u8;
    out |= bits[0];
    for bit in &bits[1..8] {
        out <<= 1;
        out |= bit;
    }

    out
}

fn u16_to_bits(inp: u16) -> [u8; 16] {
    let mut out = [0; 16];
    for i in 0..16 {
        out[15-i] = ((inp & MASKS[i]) >> i) as u8
    }

    out
}

fn pixel_row_to_str(row: [bool; PIXELS]) -> String {
    let mut out = String::new();
    for pixel in row {
        if pixel {
            out.push('1')
        } else {
            out.push('0')
        }
    }

    out
}

pub fn encode(data: String) -> Pixels {
    let mut modules = Vec::new();
    modules.extend_from_slice(&MODE);
    modules.extend_from_slice(&pad_left(u8_to_bits(data.len() as u8).to_vec(), 0, 9));

    let chunks = data.as_bytes().chunks_exact(2);

    for chunk in chunks.clone() {
        let n1 = ALPHAS.get(&char::from_u32(chunk[0] as u32).unwrap()).unwrap();
        let n2 = ALPHAS.get(&char::from_u32(chunk[1] as u32).unwrap()).unwrap();

        let n = (n1 * 45) + n2;
        modules.extend_from_slice(&u16_to_bits(n)[5..]);
    }

    if !chunks.remainder().is_empty() {
        let bits = u8_to_bits(*ALPHAS.get(&char::from_u32(chunks.remainder()[0] as u32).unwrap()).unwrap() as u8);
        modules.extend_from_slice(&bits[2..])
    }

    let n = modules.len();

    match n {
        0..=859 => pad_right_mut(&mut modules, 0, (n + 11) & !7),
        860..=864 => pad_right_mut(&mut modules, 0, 864 - n),
        _ => {}
    }

    if modules.len() < 864 {
        let n1 = u8_to_bits(236);
        let n2 = u8_to_bits(17);

        let n = (864 - modules.len()) / 8;
        let odd = n % 2 != 0;

        for _ in (0..(n/2)*2).step_by(2) {
            modules.extend_from_slice(&n1);
            modules.extend_from_slice(&n2);
        }

        if odd {
            modules.extend_from_slice(&n1);
        }
    }

    let coeffs: Vec<_> = modules.chunks_exact(8).map(|chunk| bits_to_u8(chunk.try_into().unwrap())).collect();

    let enc = Encoder::new(26);
    let data = enc.encode(coeffs.as_slice());

    modules = Vec::new();
    for coeff in &data[..] {
        modules.extend_from_slice(&u8_to_bits(*coeff));
    }

    modules.extend_from_slice(&[0; 7]);

    let mut pixels = [[false; PIXELS]; PIXELS];

    init_pixels(&mut pixels);

    for ((x, y), bit) in POS.iter().zip(modules.iter()) {
        pixels[*y][*x] = if *bit == 1 { true } else { false }
    }

    let mut masks = [[[true; PIXELS]; PIXELS]; 8];

    masks[0] = apply_mask(pixels, |x, y| (x + y) % 2 == 0);
    masks[1] = apply_mask(pixels, |_, y| y % 2 == 0);
    masks[2] = apply_mask(pixels, |x, _| x % 3 == 0);
    masks[3] = apply_mask(pixels, |x, y| (x + y) % 3 == 0);
    masks[4] = apply_mask(pixels, |x, y| (y / 2 + x / 3) % 2 == 0);
    masks[5] = apply_mask(pixels, |x, y| (((x * y) % 2) + ((x * y) % 3)) == 0);
    masks[6] = apply_mask(pixels, |x, y| (((x * y) % 2) + ((x * y) % 3)) % 2 == 0);
    masks[7] = apply_mask(pixels, |x, y| (((x + y) % 2) + ((x * y) % 3)) % 2 == 0);

    let mut penalties = [0usize; 8];
    for i in 0..8 {
        penalties[i] = calc_penalty(masks[i]);
    }

    let min_penalty_idx = penalties.iter().enumerate().min_by(|(_, a), (_, b)| a.cmp(b)).map(|(index, _)| index).unwrap();

    pixels = masks[min_penalty_idx];

    apply_format_bits(&mut pixels, min_penalty_idx);

    pixels
}

fn init_pixels(pixels: &mut Pixels) {
    for i in 0..7 {
        pixels[0][i] = true;
        pixels[0][PIXELS - 1 - i] = true;

        pixels[6][i] = true;
        pixels[6][PIXELS - 1 - i] = true;

        pixels[PIXELS - 7][i] = true;
        pixels[PIXELS - 1][i] = true;
    }

    for i in 0..5 {
        pixels[1 + i][0] = true;
        pixels[1 + i][6] = true;
        pixels[1 + i][PIXELS - 7] = true;
        pixels[1 + i][PIXELS - 1] = true;
        pixels[31 + i][0] = true;
        pixels[31 + i][6] = true;
        pixels[28][28 + i] = true;
        pixels[28 + i][28] = true;
        pixels[32][32 - i] = true;
        pixels[32 - i][32] = true;
    }

    for i in 8..29 {
        if i % 2 == 0 {
            pixels[i][6] = true;
            pixels[6][i] = true;
        }
    }

    for i in 0..3 {
        for j in 0..3 {
            pixels[2 + j][2 + i] = true;
            pixels[2 + j][32 + i] = true;
            pixels[32 + j][2 + i] = true;
        }
    }

    pixels[29][8] = true;
    pixels[PIXELS - 7][PIXELS - 7] = true;
}

fn apply_mask<F>(pixels: Pixels, f: F) -> Pixels
where F: Fn(usize, usize) -> bool
{
    let mut out = pixels;
    for (x, y) in POS {
        if f(x, y) {
            out[y][x] = !pixels[y][x];
        }
    }

    out
}

fn apply_format_bits(pixels: &mut Pixels, mask_id: usize) {
    let format_bits = MASK_FORMAT_STRINGS[mask_id];

    for ((x, y), bit) in FORMAT_POS_1.iter().zip(format_bits.iter()) {
        pixels[*y][*x] = if *bit == 1 { true } else { false }
    }

    for ((x, y), bit) in FORMAT_POS_2.iter().zip(format_bits.iter()) {
        pixels[*y][*x] = if *bit == 1 { true } else { false }
    }
}

fn rotate(pixels: Pixels) -> Pixels {
    let mut out_pixels = [[false; PIXELS]; PIXELS];

    for i in 0..PIXELS {
        for j in 0..PIXELS {
            out_pixels[i][j] = pixels[j][i]
        }
    }

    out_pixels
}

fn calc_penalty(pixels: Pixels) -> usize {
    let mut penalty = 0;

    for row in pixels {
        let mut recent = row[0];
        penalty += row.iter()
            .fold(vec![1usize], |acc, &bit| {
                let mut ret = acc;

                if bit == recent {
                    *ret.last_mut().unwrap() += 1;
                } else {
                    recent = bit;
                    ret.push(1)
                }

                ret
            }).iter()
            .filter(|&&num_same| num_same >= 5)
            .map(|num_same| num_same - 2)
            .fold(0, |acc, penalty| acc + penalty);
    }

    for col in rotate(pixels) {
        let mut recent = col[0];
        penalty += col.iter()
            .fold(vec![1usize], |acc, &bit| {
                let mut ret = acc;

                if bit == recent {
                    *ret.last_mut().unwrap() += 1;
                } else {
                    recent = bit;
                    ret.push(1)
                }

                ret
            }).iter()
            .filter(|&&num_same| num_same >= 5)
            .map(|num_same| num_same - 2)
            .fold(0, |acc, penalty| acc + penalty);
    }

    for x in 0..PIXELS-1 {
        for y in 0..PIXELS-1 {
            if pixels[y][x] == pixels[y][x+1] && pixels[y][x+1] == pixels[y+1][x] && pixels[y+1][x] == pixels[y+1][x+1] {
                penalty += 3
            }
        }
    }

    for row in pixels {
        let row_str = pixel_row_to_str(row);
        if row_str.contains("10111010000") || row_str.contains("00001011101") {
            penalty += 40
        }
    }

    for col in rotate(pixels) {
        let col_str = pixel_row_to_str(col);
        if col_str.contains("10111010000") || col_str.contains("00001011101") {
            penalty += 40
        }
    }

    let mut num_dark = 0i32;
    for row in pixels {
        for pixel in row {
            if pixel {
                num_dark += 1;
            }
        }
    }

    let percent = (num_dark / 1369) * 100;
    let lower = (((percent / 5) * 5) - 50).abs();
    let upper = ((((percent / 5) + 1) * 5) - 50).abs();

    penalty += (lower.min(upper) * 10) as usize;

    println!("penalty = {}", penalty);

    penalty
}

const BLACK: Rgb<u8> = Rgb([0, 0, 0]);
const WHITE: Rgb<u8> = Rgb([255, 255, 255]);

pub fn to_png(pixels: Pixels, file: String) {
    let mut img: RgbImage = ImageBuffer::from_pixel(410, 410, WHITE);

    for (i, row) in pixels.iter().enumerate() {
        for (j, &bit) in row.iter().enumerate() {
            for x in 0..10 {
                for y in 0..10 {
                    img.put_pixel((20 + (j * 10) + x) as u32, (20 + (i * 10) + y) as u32, if bit { BLACK } else { WHITE })
                }
            }
        }
    }

    img.save(file).unwrap();
}