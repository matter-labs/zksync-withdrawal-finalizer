use ethers::types::U256;
use num::{bigint::ToBigInt, rational::Ratio, traits::Pow, BigUint};
use sqlx::types::BigDecimal;

/// Converts `U256` into the corresponding `BigUint` value.
fn u256_to_biguint(value: U256) -> BigUint {
    let mut bytes = [0u8; 32];
    value.to_little_endian(&mut bytes);
    BigUint::from_bytes_le(&bytes)
}

pub(crate) fn u256_to_big_decimal(value: U256) -> BigDecimal {
    let ratio = Ratio::new_raw(u256_to_biguint(value), BigUint::from(1u8));
    ratio_to_big_decimal(&ratio, 80)
}

fn ratio_to_big_decimal(num: &Ratio<BigUint>, precision: usize) -> BigDecimal {
    let bigint = round_precision_raw_no_div(num, precision)
        .to_bigint()
        .unwrap();
    BigDecimal::new(bigint, precision as i64)
}

fn round_precision_raw_no_div(num: &Ratio<BigUint>, precision: usize) -> BigUint {
    let ten_pow = BigUint::from(10u32).pow(precision);
    (num * ten_pow).round().to_integer()
}
