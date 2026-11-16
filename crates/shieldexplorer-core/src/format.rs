/// Tamaño legible: `512 B`, `1.5 KiB`, `220 MiB`.
pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    const UNITS: [&str; 3] = ["KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    loop {
        value /= 1024.0;
        if value < 1024.0 || unit == UNITS.len() - 1 {
            break;
        }
        unit += 1;
    }
    let tenth = (value * 10.0).round() / 10.0;
    if tenth >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{tenth:.1} {}", UNITS[unit])
    }
}

/// Permisos al estilo de `ls -l`: `drwxr-xr-x`, `-rw-r--r--`, `lrwxrwxrwx`, `drwxrwxrwt`.
pub fn mode_string(mode: u32) -> String {
    let kind = match mode & 0o170_000 {
        0o040_000 => 'd',
        0o120_000 => 'l',
        0o060_000 => 'b',
        0o020_000 => 'c',
        0o010_000 => 'p',
        0o140_000 => 's',
        _ => '-',
    };
    let mut out = String::with_capacity(10);
    out.push(kind);
    // (desplazamiento del trío, bit especial, letra con x, letra sin x)
    for (shift, special, with_x, without_x) in [
        (6, 0o4000, 's', 'S'),
        (3, 0o2000, 's', 'S'),
        (0, 0o1000, 't', 'T'),
    ] {
        let bits = (mode >> shift) & 0o7;
        out.push(if bits & 4 != 0 { 'r' } else { '-' });
        out.push(if bits & 2 != 0 { 'w' } else { '-' });
        out.push(match (mode & special != 0, bits & 1 != 0) {
            (true, true) => with_x,
            (true, false) => without_x,
            (false, true) => 'x',
            (false, false) => '-',
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_sizes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1536), "1.5 KiB");
        assert_eq!(format_size(220_331), "215 KiB");
        assert_eq!(format_size(99 * 1024 + 970), "99.9 KiB");
        assert_eq!(format_size(99 * 1024 + 1000), "100 KiB");
        assert_eq!(format_size(6_254_387_200), "5.8 GiB");
        assert_eq!(format_size(u64::MAX), "17179869184 GiB");
    }

    #[test]
    fn formats_modes() {
        assert_eq!(mode_string(0o040_755), "drwxr-xr-x");
        assert_eq!(mode_string(0o100_644), "-rw-r--r--");
        assert_eq!(mode_string(0o120_777), "lrwxrwxrwx");
        assert_eq!(mode_string(0o041_777), "drwxrwxrwt");
        assert_eq!(mode_string(0o104_755), "-rwsr-xr-x");
        assert_eq!(mode_string(0o102_644), "-rw-r-Sr--");
    }
}
