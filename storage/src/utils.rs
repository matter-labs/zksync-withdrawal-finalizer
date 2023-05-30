use bigdecimal::BigDecimal;
use ethers::types::U256;
use num::{bigint::ToBigInt, rational::Ratio, traits::Pow, BigUint};

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

/// Converts `BigUint` value into the corresponding `U256` value.
pub fn biguint_to_u256(value: BigUint) -> U256 {
    let bytes = value.to_bytes_le();
    U256::from_little_endian(&bytes)
}

/// Converts `BigDecimal` value into the corresponding `U256` value.
pub fn bigdecimal_to_u256(value: BigDecimal) -> U256 {
    let bigint = value.with_scale(0).into_bigint_and_exponent().0;
    biguint_to_u256(bigint.to_biguint().unwrap())
}
