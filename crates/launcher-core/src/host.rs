use crate::Kind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    Windows,
    Linux,
    MacOs,
}

impl Host {
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }
}

/// CPU architecture of the host. Only macOS ships builds for both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    Aarch64,
}

impl Arch {
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(target_arch = "aarch64") {
            Self::Aarch64
        } else {
            Self::X86_64
        }
    }
}

impl Kind {
    #[must_use]
    pub const fn supported_on(self, host: Host) -> bool {
        match host {
            Host::Windows => !matches!(self, Self::Macutl),
            Host::Linux => matches!(self, Self::Neoutl | Self::Freya),
            Host::MacOs => matches!(self, Self::Freya | Self::Macutl),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Host, Kind};

    #[test]
    fn macos_supports_freya_and_macutl_only() {
        for kind in [Kind::Aviutl, Kind::Aviutl2, Kind::Ymm4, Kind::Neoutl] {
            assert!(!kind.supported_on(Host::MacOs));
        }
        assert!(Kind::Freya.supported_on(Host::MacOs));
        assert!(Kind::Macutl.supported_on(Host::MacOs));
    }

    #[test]
    fn macutl_is_macos_only() {
        assert!(!Kind::Macutl.supported_on(Host::Windows));
        assert!(!Kind::Macutl.supported_on(Host::Linux));
    }
}
