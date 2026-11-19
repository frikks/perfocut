//! Профили станков: готовые настройки G-code для популярных систем управления.

use crate::params::Params;

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
