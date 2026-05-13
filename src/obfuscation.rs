use std::fmt::Write;

use sha2::{Digest, Sha256};

include!(concat!(env!("OUT_DIR"), "/obfuscation_key.rs"));

const ADMIN_PASSWORD_SHA256: &str =
    "27aa85d9579166a014c206c41971b23eadb20ac6a42aca4a88f7e64cf25fa6a2";

pub const fn decode_usize(value: usize) -> usize {
    value ^ ADDRESS_KEY
}

pub const fn decode_isize(value: isize) -> isize {
    value ^ ADDRESS_KEY_ISIZE
}

pub fn admin_password_is_valid(value: &str) -> bool {
    admin_password_hash(value).eq_ignore_ascii_case(ADMIN_PASSWORD_SHA256)
}

pub fn admin_password_hash_is_valid(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case(ADMIN_PASSWORD_SHA256)
}

pub fn admin_password_config_value(value: &str) -> Option<String> {
    if admin_password_hash_is_valid(value) {
        Some(value.trim().to_ascii_lowercase())
    } else if admin_password_is_valid(value) {
        Some(admin_password_hash(value))
    } else {
        None
    }
}

pub fn admin_password_hash(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.trim().as_bytes());
    let digest = hasher.finalize();

    let mut output = String::with_capacity(64);
    for byte in digest {
        let _ = write!(output, "{byte:02x}");
    }
    output
}
