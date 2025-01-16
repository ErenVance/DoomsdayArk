use crate::constants::config::SECONDS_PER_DAY;
use crate::errors::ErrorCode;
use anchor_lang::prelude::*;
use anchor_safe_math::SafeMath;

pub fn to_timestamp_u64(t: i64) -> Result<u64> {
    u64::try_from(t).or(Err(ErrorCode::InvalidTimestampConversion.into()))
}

pub fn timestamp_to_days(timestamp: u64) -> Result<u32> {
    timestamp
        .safe_div(SECONDS_PER_DAY)
        .or(Err(ErrorCode::InvalidTimestampConversion.into()))
        .map(|days| days as u32)
}
