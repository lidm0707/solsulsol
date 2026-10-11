//! Generate and validate a PromptPay payload from input:
//!   cargo run -p pos-promptpay --example gen -- 0812345678 150.00

use pos_promptpay::payload::{build_from_raw, crc16};
use rust_decimal::Decimal;

const PHONE_DIGITS: usize = 10;
const NATIONAL_ID_DIGITS: usize = 13;
const WALLET_DIGITS: usize = 16;
const CRC_LEN: usize = 4;
const PAYLOAD_FORMAT_PREFIX: &str = "000201";
const CURRENCY_TAG_THB: &str = "5303764";
const USAGE: &str =
    "usage: cargo run -p pos-promptpay --example gen -- <phone|national-id|wallet> <amount>";

fn check_target(raw: &str) -> Result<(), String> {
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    let ok = digits.len() == PHONE_DIGITS
        || digits.len() == NATIONAL_ID_DIGITS
        || digits.len() == WALLET_DIGITS;
    if ok {
        Ok(())
    } else {
        Err(format!(
            "target has {len} digits, expected {PHONE_DIGITS} (phone), {NATIONAL_ID_DIGITS} (national id) or {WALLET_DIGITS} (e-wallet): {raw}",
            len = digits.len()
        ))
    }
}

fn validate(payload: &str) -> Result<(), String> {
    if payload.len() < CRC_LEN {
        return Err("payload too short".into());
    }
    let (body, crc) = payload.split_at(payload.len() - CRC_LEN);
    if !body.starts_with(PAYLOAD_FORMAT_PREFIX) {
        return Err("missing payload format indicator".into());
    }
    if !body.contains(CURRENCY_TAG_THB) {
        return Err("missing THB currency tag".into());
    }
    let expected = crc16(body);
    if expected != crc {
        return Err(format!("crc mismatch: expected {expected} got {crc}"));
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("{USAGE}");
        std::process::exit(2);
    }
    let target = &args[1];
    let amount: Decimal = match args[2].parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("invalid amount {}: {e}", args[2]);
            std::process::exit(2);
        }
    };
    if let Err(e) = check_target(target) {
        eprintln!("{e}");
        std::process::exit(2);
    }
    let payload = build_from_raw(target, amount);
    match validate(&payload) {
        Ok(()) => println!("{payload}"),
        Err(e) => {
            eprintln!("invalid payload: {e}");
            std::process::exit(1);
        }
    }
}
