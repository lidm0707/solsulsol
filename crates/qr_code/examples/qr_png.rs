//! Render a payload to a QR PNG file: `cargo run -p pos-qr-code --example qr_png -- <payload> <out.png>`

use std::io::Write as _;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let payload = args.next().unwrap_or_else(|| String::from("payload"));
    let out_path = args.next().unwrap_or_else(|| String::from("qr.png"));
    let png = pos_qr_code::encode::encode_png(&payload)?;
    let mut f = std::fs::File::create(&out_path)?;
    f.write_all(&png)?;
    println!("{out_path}");
    Ok(())
}
