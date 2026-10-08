//! PromptPay target resolution from raw bank/phone/id input.

pub const PHONE_LOCAL_LEN: usize = 10;
pub const NATIONAL_ID_LEN: usize = 13;
pub const WALLET_LEN: usize = 15;
pub const PHONE_INTL_PREFIX: &str = "0066";

pub fn target(raw: &str) -> String {
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    match digits.len() {
        PHONE_LOCAL_LEN if digits.starts_with('0') => {
            format!("{PHONE_INTL_PREFIX}{}", &digits[1..])
        }
        NATIONAL_ID_LEN | WALLET_LEN => digits,
        _ => digits,
    }
}
