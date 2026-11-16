//! Estado de los discos: unidades montadas con su uso, leído con `df`.

use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disk {
    pub device: String,
    pub fstype: String,
    pub size: u64,
    pub used: u64,
    pub avail: u64,
    pub mount: PathBuf,
}

impl Disk {
    /// Porcentaje usado, calculado como lo hace `df` (usado entre usado + disponible).
    pub fn percent(&self) -> u8 {
        let total = self.used.saturating_add(self.avail);
        if total == 0 {
            return 0;
        }
        (u128::from(self.used) * 100)
            .div_ceil(u128::from(total))
            .min(100) as u8
    }

    /// Unidades externas: las que el escritorio monta en `/media` o `/run/media`.
    pub fn removable(&self) -> bool {
        self.mount.starts_with("/media") || self.mount.starts_with("/run/media")
    }
}

/// Montajes que no interesan: de sistema, de snaps o de contenedores.
fn hidden_mount(mount: &str) -> bool {
    [
        "/snap",
        "/var/snap",
        "/run/user",
        "/var/lib/docker",
        "/var/lib/snapd",
        "/sys",
        "/proc",
        "/dev",
    ]
    .iter()
    .any(|p| mount == *p || mount.starts_with(&format!("{p}/")))
}

/// Interpreta la salida de `df -B1 --output=source,fstype,size,used,avail,target`.
pub fn parse_df(text: &str) -> Vec<Disk> {
    let mut disks: Vec<Disk> = text.lines().skip(1).filter_map(parse_line).collect();
    let root = std::path::Path::new("/");
    disks.sort_by(|a, b| (a.mount != root, &a.mount).cmp(&(b.mount != root, &b.mount)));
    disks
}

/// Una línea de `df`: cinco campos sin espacios y, al final, el punto de montaje (que puede
/// llevarlos).
fn parse_line(line: &str) -> Option<Disk> {
    let mut rest = line.trim_start();
    let mut fields = [""; 5];
    for field in &mut fields {
        let end = rest.find(char::is_whitespace)?;
        *field = &rest[..end];
        rest = rest[end..].trim_start();
    }
    let mount = rest.trim_end();
    if mount.is_empty() || hidden_mount(mount) {
        return None;
    }
    Some(Disk {
        device: fields[0].to_owned(),
        fstype: fields[1].to_owned(),
        size: fields[2].parse().ok()?,
        used: fields[3].parse().ok()?,
        avail: fields[4].parse().ok()?,
        mount: PathBuf::from(mount),
    })
}

/// Discos montados ahora mismo. Bloquea: llamarlo en segundo plano.
pub fn list() -> Vec<Disk> {
    let out = Command::new("df")
        .args(["-B1", "--output=source,fstype,size,used,avail,target"])
        .args([
            "-x", "tmpfs", "-x", "devtmpfs", "-x", "squashfs", "-x", "overlay", "-x", "efivarfs",
        ])
        .args([
            "-x", "ramfs", "-x", "devpts", "-x", "proc", "-x", "sysfs", "-x", "cgroup2",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    match out {
        Ok(o) if o.status.success() || !o.stdout.is_empty() => {
            parse_df(&String::from_utf8_lossy(&o.stdout))
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Filesystem     Type       1B-blocks        Used       Avail Mounted on
/dev/nvme0n1p5 ext4    500000000000 200000000000 275000000000 /
/dev/nvme0n1p1 vfat       536870912     6291456   530579456 /boot/efi
/dev/sdb1      exfat    64000000000  16000000000  48000000000 /media/sebastian/USB DISCO
/dev/loop3     squashfs   100000000   100000000           0 /snap/core/1
/dev/sda2      ext4    1000000000000 900000000000  50000000000 /home
";

    #[test]
    fn parses_df_and_hides_noise() {
        let disks = parse_df(SAMPLE);
        let mounts: Vec<_> = disks
            .iter()
            .map(|d| d.mount.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            mounts,
            ["/", "/boot/efi", "/home", "/media/sebastian/USB DISCO"]
        );
        let root = &disks[0];
        assert_eq!(
            (root.device.as_str(), root.fstype.as_str(), root.size),
            ("/dev/nvme0n1p5", "ext4", 500_000_000_000)
        );
    }

    #[test]
    fn computes_percent_like_df() {
        let disks = parse_df(SAMPLE);
        assert_eq!(disks[0].percent(), 43); // 200/(200+275) = 42.1 -> redondea hacia arriba
        assert_eq!(disks[2].percent(), 95);
        assert!(disks[3].removable() && !disks[0].removable());
    }

    #[test]
    fn empty_or_broken_output_gives_no_disks() {
        assert!(parse_df("").is_empty());
        assert!(parse_df("cabecera\nsolo dos campos\n").is_empty());
    }
}
