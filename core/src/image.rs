/// Полутоновое изображение с таблицей сумм (summed-area table) для быстрого усреднения по ячейке.
pub struct Gray {
    pub w: usize,
    pub h: usize,
    sat: Vec<f64>, // (w+1)*(h+1)
}

impl Gray {
    pub fn from_rgba(data: &[u8], w: usize, h: usize) -> Gray {
        let mut sat = vec![0.0f64; (w + 1) * (h + 1)];
        for y in 0..h {
            let mut row = 0.0;
            for x in 0..w {
                let i = (y * w + x) * 4;
                let (r, g, b, a) = if i + 3 < data.len() {
                    (data[i] as f64, data[i + 1] as f64, data[i + 2] as f64, data[i + 3] as f64 / 255.0)
                } else {
                    (255.0, 255.0, 255.0, 1.0)
                };
                let lum = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0;
                let v = lum * a + (1.0 - a); // прозрачность = белый
                row += v;
                sat[(y + 1) * (w + 1) + (x + 1)] = sat[y * (w + 1) + (x + 1)] + row;
            }
        }
        Gray { w, h, sat }
    }

    /// Среднее значение в прямоугольнике пикселей [x0,x1) x [y0,y1) (дробные границы округляются).
    pub fn mean(&self, x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
        let wf = self.w as f64;
        let hf = self.h as f64;
        let mut ax = x0.floor().clamp(0.0, wf - 1.0) as usize;
        let mut ay = y0.floor().clamp(0.0, hf - 1.0) as usize;
        let mut bx = x1.ceil().clamp(1.0, wf) as usize;
        let mut by = y1.ceil().clamp(1.0, hf) as usize;
        if bx <= ax {
            bx = ax + 1;
        }
        if by <= ay {
            by = ay + 1;
        }
        if bx > self.w {
            bx = self.w;
            ax = bx - 1;
        }
        if by > self.h {
            by = self.h;
            ay = by - 1;
        }
        let s = |x: usize, y: usize| self.sat[y * (self.w + 1) + x];
        let sum = s(bx, by) - s(ax, by) - s(bx, ay) + s(ax, ay);
        sum / (((bx - ax) * (by - ay)) as f64)
    }
}
