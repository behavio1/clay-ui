//! The JavaScript semantics the original CLI relied on, reproduced exactly where
//! they influence ranking, cache keys or output.

use serde_json::Value;
use std::cmp::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

pub type Object = serde_json::Map<String, Value>;

pub fn is_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

pub fn is_line_end(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

pub fn trim(text: &str) -> &str {
    text.trim_matches(is_space)
}

/// Byte offsets of every position where a multiline `^` matches.
pub fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, c) in text.char_indices() {
        if is_line_end(c) {
            starts.push(index + c.len_utf8());
        }
    }
    starts
}

/// Lines split as multiline `^...$` sees them.
pub fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split(is_line_end)
}

/// `str.slice(0, n)`: counted in UTF-16 units, and a split pair becomes U+FFFD when output.
pub fn slice_utf16(text: &str, count: usize) -> String {
    if text.len() <= count {
        return text.to_string();
    }
    let units: Vec<u16> = text.encode_utf16().take(count).collect();
    String::from_utf16_lossy(&units)
}

pub fn encode_uri_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

pub fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(flag)) => *flag,
        Some(Value::String(text)) => !text.is_empty(),
        Some(Value::Number(number)) => number.as_f64().map_or(true, |n| n != 0.0 && !n.is_nan()),
        Some(_) => true,
    }
}

fn number_text(number: &serde_json::Number) -> String {
    let text = number.to_string();
    text.strip_suffix(".0").map_or(text.clone(), str::to_string)
}

/// `String(value)`.
pub fn string_of(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number_text(number),
        Value::String(text) => text.clone(),
        Value::Array(items) => join(items, ","),
        Value::Object(_) => "[object Object]".into(),
    }
}

/// Template-literal text of a property that may be missing.
pub fn template(value: Option<&Value>) -> String {
    value.map_or("undefined".into(), string_of)
}

/// `array.join(separator)`; null elements become empty strings.
pub fn join(items: &[Value], separator: &str) -> String {
    items
        .iter()
        .map(|item| {
            if item.is_null() {
                String::new()
            } else {
                string_of(item)
            }
        })
        .collect::<Vec<_>>()
        .join(separator)
}

/// `value ?? fallback`.
pub fn coalesce(value: Option<&Value>, fallback: Value) -> Value {
    match value {
        None | Some(Value::Null) => fallback,
        Some(found) => found.clone(),
    }
}

/// A score as `JSON.stringify` prints it: 1 rather than 1.0.
pub fn number_value(number: f64) -> Value {
    if number.fract() == 0.0 && number.abs() < 9e15 {
        Value::from(number as i64)
    } else {
        Value::from(number)
    }
}

pub fn math_round(value: f64) -> f64 {
    let lower = value.floor();
    if value - lower >= 0.5 {
        lower + 1.0
    } else {
        lower
    }
}

/// `Number(text)`, or NaN.
pub fn number_from_text(text: &str) -> f64 {
    let body = trim(text);
    if body.is_empty() {
        return 0.0;
    }
    let lower = body.to_ascii_lowercase();
    for (prefix, radix) in [("0x", 16), ("0o", 8), ("0b", 2)] {
        if let Some(digits) = lower.strip_prefix(prefix) {
            if digits.is_empty() {
                return f64::NAN;
            }
            let mut total = 0.0;
            for digit in digits.chars() {
                match digit.to_digit(radix) {
                    Some(value) => total = total * f64::from(radix) + f64::from(value),
                    None => return f64::NAN,
                }
            }
            return total;
        }
    }
    let unsigned = body.strip_prefix(['+', '-']).unwrap_or(body);
    if unsigned == "Infinity" {
        return if body.starts_with('-') {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    let mut parts = unsigned.splitn(2, ['e', 'E']);
    let mantissa = parts.next().unwrap_or("");
    let digits_ok = mantissa.chars().all(|c| c.is_ascii_digit() || c == '.')
        && mantissa.matches('.').count() <= 1
        && mantissa.chars().any(|c| c.is_ascii_digit());
    let exponent_ok = parts.next().map_or(true, |exponent| {
        let digits = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
    });
    if digits_ok && exponent_ok {
        body.parse().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

pub fn is_safe_integer(number: f64) -> bool {
    number.is_finite() && number.fract() == 0.0 && number.abs() <= 9_007_199_254_740_991.0
}

// ICU root collation order of printable ASCII, as `localeCompare` uses it in Node.
const COLLATION: &str = " _-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$0123456789aAbBcCdDeEfFgGhHiIjJkKlLmMnNoOpPqQrRsStTuUvVwWxXyYzZ";

fn primary_weight(c: char) -> u32 {
    if c.is_ascii_control() {
        return 0;
    }
    match COLLATION.find(c.to_ascii_lowercase()) {
        Some(position) => position as u32 + 1,
        None => 1000 + c as u32,
    }
}

/// `a.localeCompare(b)` for the ASCII ids this CLI ranks: primary differences first, then lower case before upper.
pub fn locale_compare(a: &str, b: &str) -> Ordering {
    let visible = |text: &str| {
        text.chars()
            .filter(|c| primary_weight(*c) != 0)
            .collect::<Vec<_>>()
    };
    let (left, right) = (visible(a), visible(b));
    let primaries = left
        .iter()
        .map(|c| primary_weight(*c))
        .cmp(right.iter().map(|c| primary_weight(*c)));
    if primaries != Ordering::Equal {
        return primaries;
    }
    for (x, y) in left.iter().zip(right.iter()) {
        match (x.is_ascii_uppercase(), y.is_ascii_uppercase()) {
            (false, true) => return Ordering::Less,
            (true, false) => return Ordering::Greater,
            _ => {}
        }
    }
    Ordering::Equal
}

/// V8's `Math.hypot`, including its Kahan summation.
pub fn hypot(values: &[f64]) -> f64 {
    let max = values
        .iter()
        .fold(0.0_f64, |largest, value| largest.max(value.abs()));
    if max == 0.0 {
        return 0.0;
    }
    let (mut sum, mut compensation) = (0.0_f64, 0.0_f64);
    for value in values {
        let scaled = value.abs() / max;
        let summand = scaled * scaled - compensation;
        let preliminary = sum + summand;
        compensation = (preliminary - sum) - summand;
        sum = preliminary;
    }
    sum.sqrt() * max
}

/// V8's `Math.log` (fdlibm), so BM25 scores match bit for bit on every platform.
#[allow(clippy::excessive_precision)]
pub fn log(input: f64) -> f64 {
    const LN2_HI: f64 = 6.93147180369123816490e-01;
    const LN2_LO: f64 = 1.90821492927058770002e-10;
    const TWO54: f64 = 1.80143985094819840000e+16;
    const LG1: f64 = 6.666666666666735130e-01;
    const LG2: f64 = 3.999999999940941908e-01;
    const LG3: f64 = 2.857142874366239149e-01;
    const LG4: f64 = 2.222219843214978396e-01;
    const LG5: f64 = 1.818357216161805012e-01;
    const LG6: f64 = 1.531383769920937332e-01;
    const LG7: f64 = 1.479819860511658591e-01;

    let mut x = input;
    let mut hx = (x.to_bits() >> 32) as i32;
    let lx = x.to_bits() as u32;
    let mut k: i32 = 0;
    if hx < 0x0010_0000 {
        if ((hx & 0x7fff_ffff) as u32 | lx) == 0 {
            return f64::NEG_INFINITY;
        }
        if hx < 0 {
            return f64::NAN;
        }
        k -= 54;
        x *= TWO54;
        hx = (x.to_bits() >> 32) as i32;
    }
    if hx >= 0x7ff0_0000 {
        return x + x;
    }
    k += (hx >> 20) - 1023;
    hx &= 0x000f_ffff;
    let i = (hx + 0x95f64) & 0x10_0000;
    let high = (hx | (i ^ 0x3ff0_0000)) as u32 as u64;
    x = f64::from_bits((high << 32) | (x.to_bits() & 0xffff_ffff));
    k += i >> 20;
    let f = x - 1.0;
    let dk = f64::from(k);
    if (0x000f_ffff & (2 + hx)) < 3 {
        if f == 0.0 {
            return if k == 0 {
                0.0
            } else {
                dk * LN2_HI + dk * LN2_LO
            };
        }
        let r = f * f * (0.5 - 0.33333333333333333 * f);
        return if k == 0 {
            f - r
        } else {
            dk * LN2_HI - ((r - dk * LN2_LO) - f)
        };
    }
    let s = f / (2.0 + f);
    let z = s * s;
    let w = z * z;
    let t1 = w * fused(w, fused(w, LG6, LG4), LG2);
    let t2 = z * fused(w, fused(w, fused(w, LG7, LG5), LG3), LG1);
    let r = t2 + t1;
    if ((hx - 0x6147a) | (0x6b851 - hx)) > 0 {
        let hfsq = 0.5 * f * f;
        if k == 0 {
            f - fused(-s, hfsq + r, hfsq)
        } else {
            fused(dk, LN2_HI, -((hfsq - fused(s, hfsq + r, dk * LN2_LO)) - f))
        }
    } else if k == 0 {
        fused(-s, f - r, f)
    } else {
        fused(dk, LN2_HI, -(fused(s, f - r, -(dk * LN2_LO)) - f))
    }
}

/// `a * b + c` the way V8's C++ is compiled: clang fuses it into one operation on arm64 and not on x86-64.
#[inline]
fn fused(a: f64, b: f64, c: f64) -> f64 {
    if cfg!(target_arch = "aarch64") {
        a.mul_add(b, c)
    } else {
        a * b + c
    }
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const BASE64_URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn encode(bytes: &[u8], alphabet: &[u8; 64], pad: bool) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for index in 0..4 {
            if index <= chunk.len() {
                out.push(alphabet[((n >> (18 - 6 * index)) & 63) as usize] as char);
            } else if pad {
                out.push('=');
            }
        }
    }
    out
}

pub fn base64_encode(bytes: &[u8]) -> String {
    encode(bytes, BASE64, true)
}

pub fn base64url_encode(bytes: &[u8]) -> String {
    encode(bytes, BASE64_URL, false)
}

/// `Buffer.from(text, 'base64')`: both alphabets, unknown characters skipped, stops at padding.
pub fn base64_decode(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut buffer, mut bits) = (0_u32, 0_u32);
    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            _ => continue,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    out
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}

/// `new Date().toISOString()`.
pub fn iso_now() -> String {
    iso_at(now_millis())
}

pub fn iso_at(millis: i64) -> String {
    let days = millis.div_euclid(86_400_000);
    let rest = millis.rem_euclid(86_400_000);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rest / 3_600_000,
        rest / 60_000 % 60,
        rest / 1000 % 60,
        rest % 1000
    )
}

/// Milliseconds for the ISO instants this CLI stores; None when the text is anything else.
pub fn parse_iso(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() < 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || !text.ends_with('Z')
    {
        return None;
    }
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let fraction = match text.get(19..text.len() - 1)? {
        "" => 0,
        part => {
            let digits = part.strip_prefix('.')?;
            format!("{digits:0<3}").get(..3)?.parse::<i64>().ok()?
        }
    };
    let (month, day) = (number(5..7)?, number(8..10)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(
        days_from_civil(number(0..4)?, month, day) * 86_400_000
            + number(11..13)? * 3_600_000
            + number(14..16)? * 60_000
            + number(17..19)? * 1000
            + fraction,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Produced by `Math.log(x)` in Node 22 on arm64; x86-64 Node differs in the last bit by design.
    #[cfg(target_arch = "aarch64")]
    #[test]
    fn log_matches_v8_bit_for_bit() {
        for (x, bits) in LOG_GOLDEN {
            assert_eq!(log(*x).to_bits(), *bits, "log({x})");
        }
    }

    #[test]
    fn collation_matches_node_samples() {
        for (a, b, expected) in [
            ("a/b", "a-b", Ordering::Greater),
            ("a_b", "a-b", Ordering::Less),
            ("A", "a", Ordering::Greater),
            ("a", "B", Ordering::Less),
            ("21st/x", "a/x", Ordering::Less),
            ("ab", "a", Ordering::Greater),
        ] {
            assert_eq!(locale_compare(a, b), expected, "{a} vs {b}");
        }
    }

    #[test]
    fn parses_numbers_like_javascript() {
        for (text, expected) in [
            ("5", 5.0),
            (" 5 ", 5.0),
            ("0x10", 16.0),
            ("1e3", 1000.0),
            ("", 0.0),
            ("+7", 7.0),
        ] {
            assert_eq!(number_from_text(text), expected);
        }
        for text in ["abc", "1.2.3", "inf", "nan", "infinity"] {
            assert!(number_from_text(text).is_nan(), "{text}");
        }
    }

    #[test]
    fn iso_round_trips() {
        let now = iso_now();
        assert_eq!(now.len(), 24);
        assert!((parse_iso(&now).unwrap() - now_millis()).abs() < 5000);
        assert_eq!(
            parse_iso("2026-11-09T00:00:00.000Z"),
            Some(1_794_182_400_000)
        );
    }

    #[test]
    fn base64_is_lossless() {
        let bytes: Vec<u8> = (0..=255).collect();
        assert_eq!(base64_decode(&base64_encode(&bytes)), bytes);
        assert_eq!(base64_encode(b"ab"), "YWI=");
        assert_eq!(base64url_encode(&[0xfb, 0xff]), "-_8");
    }

    #[cfg(target_arch = "aarch64")]
    const LOG_GOLDEN: &[(f64, u64)] = &include!("js_log_golden.in");
}
