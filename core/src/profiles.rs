//! Профили станков: готовые настройки G-code и пользовательский JSON.

use crate::params::Params;
use serde::Deserialize;

#[derive(Clone, Debug)]
pub struct Profile {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub on: &'static str,
    pub off: &'static str,
    pub header: &'static str,
    pub footer: &'static str,
    pub arcs: bool,
    pub decimals: u32,
}

pub static PROFILES: &[Profile] = &[
    Profile {
        id: "cypcut",
        name: "CypCut",
        desc: "Китайские лазерные станки, M07/M08, дуги G02",
        on: "M07",
        off: "M08",
        header: "G21\nG90\nG17",
        footer: "M30",
        arcs: true,
        decimals: 3,
    },
    Profile {
        id: "ncstudio",
        name: "NCStudio",
        desc: "NCStudio, M03/M05, дуги G02",
        on: "M03",
        off: "M05",
        header: "G21\nG90",
        footer: "M30",
        arcs: true,
        decimals: 3,
    },
    Profile {
        id: "grbl",
        name: "GRBL",
        desc: "GRBL и лазерные модули, M3/M5, только линейные Move",
        on: "M3",
        off: "M5",
        header: "G21\nG90\nG17",
        footer: "M5\nG0 X0 Y0",
        arcs: false,
        decimals: 3,
    },
    Profile {
        id: "linuxcnc",
        name: "LinuxCNC",
        desc: "LinuxCNC, M3/M5, толеранс контура G64",
        on: "M3",
        off: "M5",
        header: "G21\nG90\nG17\nG64 P0.05",
        footer: "M2",
        arcs: true,
        decimals: 4,
    },
];

impl Profile {
    pub fn apply_to(&self, p: &mut Params) {
        p.gc_on = self.on.into();
        p.gc_off = self.off.into();
        p.gc_header = self.header.into();
        p.gc_footer = self.footer.into();
        p.gc_arcs = self.arcs;
        p.gc_decimals = self.decimals;
    }
}

pub fn find(id: &str) -> Option<&'static Profile> {
    PROFILES.iter().find(|p| p.id == id)
}

/// Пользовательский профиль из JSON: можно добавить свой станок без пересборки.
#[derive(Clone, Debug, Deserialize)]
pub struct CustomProfile {
    pub name: String,
    #[serde(default = "default_on")]
    pub on: String,
    #[serde(default = "default_off")]
    pub off: String,
    #[serde(default = "default_header")]
    pub header: String,
    #[serde(default = "default_footer")]
    pub footer: String,
    #[serde(default)]
    pub arcs: bool,
    #[serde(default = "default_decimals")]
    pub decimals: u32,
}

fn default_on() -> String {
    "M3".into()
}
fn default_off() -> String {
    "M5".into()
}
fn default_header() -> String {
    "G21\nG90\nG17".into()
}
fn default_footer() -> String {
    "M30".into()
}
fn default_decimals() -> u32 {
    3
}

impl CustomProfile {
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    pub fn apply_to(&self, p: &mut Params) {
        p.gc_on = self.on.clone();
        p.gc_off = self.off.clone();
        p.gc_header = self.header.clone();
        p.gc_footer = self.footer.clone();
        p.gc_arcs = self.arcs;
        p.gc_decimals = self.decimals;
    }
}
