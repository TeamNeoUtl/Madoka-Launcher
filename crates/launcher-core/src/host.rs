use crate::Kind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    Windows,
    Linux,
}

impl Host {
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

impl Kind {
    pub const fn supported_on(self, host: Host) -> bool {
        match host {
            Host::Windows => true,
            Host::Linux => matches!(self, Self::Neoutl | Self::Freya),
        }
    }
}
