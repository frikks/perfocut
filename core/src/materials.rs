//! Справочник материалов: плотность и ориентировочная цена за кг.
//! Значения можно править под свой регион и поставщика.

use crate::params::Params;

#[derive(Clone, Debug)]
pub struct Material {
    pub id: &'static str,
    pub name: &'static str,
    pub density: f64,
    pub price_kg: f64,
    pub thickness: f64,
    pub kerf: f64,
    pub cut_speed: f64,
    pub pierce_time: f64,
}

pub static MATERIALS: &[Material] = &[
    Material {
        id: "steel1",
        name: "Сталь 1 мм",
        density: 7.85,
        price_kg: 95.0,
        thickness: 1.0,
        kerf: 0.08,
        cut_speed: 4000.0,
        pierce_time: 0.3,
    },
    Material {
        id: "steel2",
        name: "Сталь 2 мм",
        density: 7.85,
        price_kg: 95.0,
        thickness: 2.0,
        kerf: 0.10,
        cut_speed: 2500.0,
        pierce_time: 0.5,
    },
    Material {
        id: "steel3",
        name: "Сталь 3 мм",
        density: 7.85,
        price_kg: 95.0,
        thickness: 3.0,
        kerf: 0.12,
        cut_speed: 1800.0,
        pierce_time: 0.7,
    },
    Material {
        id: "inox15",
        name: "Нерж. 1,5 мм",
        density: 7.95,
        price_kg: 240.0,
        thickness: 1.5,
        kerf: 0.10,
        cut_speed: 3000.0,
        pierce_time: 0.4,
    },
    Material {
        id: "aluminium",
        name: "Алюминий 2 мм",
        density: 2.70,
        price_kg: 210.0,
        thickness: 2.0,
        kerf: 0.10,
        cut_speed: 5000.0,
        pierce_time: 0.3,
    },
    Material {
        id: "galvanized",
        name: "Оцинковка 1 мм",
        density: 7.85,
        price_kg: 110.0,
        thickness: 1.0,
        kerf: 0.08,
        cut_speed: 4000.0,
        pierce_time: 0.3,
    },
    Material {
        id: "copper",
        name: "Медь 1 мм",
        density: 8.96,
        price_kg: 780.0,
        thickness: 1.0,
        kerf: 0.08,
        cut_speed: 2500.0,
        pierce_time: 0.4,
    },
];

impl Material {
    pub fn apply_to(&self, p: &mut Params) {
        p.density = self.density;
        p.price_kg = self.price_kg;
        p.thickness = self.thickness;
        p.kerf = self.kerf;
        p.cut_speed = self.cut_speed;
        p.pierce_time = self.pierce_time;
    }
}

pub fn find(id: &str) -> Option<&'static Material> {
    MATERIALS.iter().find(|m| m.id == id)
}
