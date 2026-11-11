//! Pure-Rust formatting helpers.

#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[must_use]
pub fn format_uptime(ms: u64) -> String {
    let total_secs = ms / 1000;
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    let mut out = String::new();
    if days > 0 {
        out.push_str(&format!("{days}d "));
    }
    if hours > 0 || days > 0 {
        out.push_str(&format!("{hours}h "));
    }
    if mins > 0 || hours > 0 || days > 0 {
        out.push_str(&format!("{mins}m "));
    }
    out.push_str(&format!("{secs}s ({ms} ms)"));
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HexRow {
    pub offset_str: String,
    pub hex_left: String,
    pub hex_right: String,
    pub ascii: String,
}

#[must_use]
pub fn format_hex_rows(bytes: &[u8], max_bytes: usize) -> (Vec<HexRow>, usize) {
    if bytes.is_empty() {
        return (Vec::new(), 0);
    }
    let len = bytes.len().min(max_bytes);
    let mut rows = Vec::new();

    for (chunk_idx, chunk) in bytes[..len].chunks(16).enumerate() {
        let offset = chunk_idx * 16;
        let offset_str = format!("{offset:06X}");

        let mut left_bytes = Vec::new();
        let mut right_bytes = Vec::new();
        let mut ascii = String::new();

        for (i, b) in chunk.iter().enumerate() {
            let hex = format!("{b:02X}");
            if i < 8 {
                left_bytes.push(hex);
            } else {
                right_bytes.push(hex);
            }
            if (32..=126).contains(b) {
                ascii.push(*b as char);
            } else {
                ascii.push('·');
            }
        }

        while left_bytes.len() < 8 {
            left_bytes.push("  ".to_string());
        }
        while right_bytes.len() < 8 {
            right_bytes.push("  ".to_string());
        }

        rows.push(HexRow {
            offset_str,
            hex_left: left_bytes.join(" "),
            hex_right: right_bytes.join(" "),
            ascii,
        });
    }

    (rows, bytes.len().saturating_sub(max_bytes))
}

#[must_use]
pub fn render_hex_dump(bytes: &[u8], max_bytes: usize) -> String {
    let (rows, truncated) = format_hex_rows(bytes, max_bytes);
    if rows.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for row in rows {
        out.push_str(&format!(
            "{}:  {}  {} |{}|\n",
            row.offset_str.to_lowercase(),
            row.hex_left.to_lowercase(),
            row.hex_right.to_lowercase(),
            row.ascii.replace('·', ".")
        ));
    }
    if truncated > 0 {
        out.push_str(&format!("... ({truncated} more bytes truncated in preview)"));
    }
    out
}

#[must_use]
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn parse_hex(text: &str) -> Result<Vec<u8>, String> {
    let clean = text
        .replace("0x", "")
        .replace("0X", "")
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect::<String>();

    if clean.is_empty() {
        return Ok(Vec::new());
    }

    if clean.len() % 2 != 0 {
        return Err("Hex string must contain an even number of characters".into());
    }

    (0..clean.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&clean[i..i + 2], 16)
                .map_err(|e| format!("Invalid hex at position {i}: {e}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_hex_rows() {
        let (rows, rem) = format_hex_rows(b"Hello, World!\x00\x01\x02", 32);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].offset_str, "000000");
        assert_eq!(rows[0].ascii, "Hello, World!···");
        assert_eq!(rem, 0);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(2048), "2.0 KB");
        assert_eq!(format_bytes(1024 * 1024 * 2 + 512 * 1024), "2.50 MB");
    }

    #[test]
    fn test_format_uptime() {
        assert_eq!(format_uptime(5000), "5s (5000 ms)");
        assert_eq!(format_uptime(65000), "1m 5s (65000 ms)");
        assert_eq!(format_uptime(3665000), "1h 1m 5s (3665000 ms)");
    }

    #[test]
    fn test_hex_dump_and_parse() {
        let data = b"Hello, world!";
        let dump = render_hex_dump(data, 100);
        assert!(dump.contains("Hello, world!"));

        let hex = bytes_to_hex(&[0x12, 0xab, 0xff]);
        assert_eq!(hex, "12 ab ff");

        let parsed = parse_hex("12 ab ff").unwrap();
        assert_eq!(parsed, vec![0x12, 0xab, 0xff]);

        let parsed2 = parse_hex("0x12, 0xAB:0xFF").unwrap();
        assert_eq!(parsed2, vec![0x12, 0xab, 0xff]);
    }
}
