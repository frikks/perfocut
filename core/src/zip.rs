//! Простейший ZIP (метод «без сжатия»), достаточный для файла заказа и пакета форматов.

fn crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    for i in 0..256u32 {
        let mut c = i;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        t[i as usize] = c;
    }
    t
}

pub fn crc32(data: &[u8]) -> u32 {
    let t = crc_table();
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = t[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

#[derive(Default)]
pub struct ZipWriter {
    buf: Vec<u8>,
    central: Vec<u8>,
    count: u16,
}

fn u16le(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn u32le(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

impl ZipWriter {
    pub fn add(&mut self, name: &str, data: &[u8]) {
        let crc = crc32(data);
        let off = self.buf.len() as u32;
        let nb = name.as_bytes();
        let (time, date) = (0u16, (46u16 << 9) | (1 << 5) | 1); // 2026-01-01
        let b = &mut self.buf;
        u32le(b, 0x0403_4b50);
        u16le(b, 20);
        u16le(b, 0x0800); // имена в UTF-8
        u16le(b, 0);
        u16le(b, time);
        u16le(b, date);
        u32le(b, crc);
        u32le(b, data.len() as u32);
        u32le(b, data.len() as u32);
        u16le(b, nb.len() as u16);
        u16le(b, 0);
        b.extend_from_slice(nb);
        b.extend_from_slice(data);
        let c = &mut self.central;
        u32le(c, 0x0201_4b50);
        u16le(c, 20);
        u16le(c, 20);
        u16le(c, 0x0800);
        u16le(c, 0);
        u16le(c, time);
        u16le(c, date);
        u32le(c, crc);
        u32le(c, data.len() as u32);
        u32le(c, data.len() as u32);
        u16le(c, nb.len() as u16);
        u16le(c, 0);
        u16le(c, 0);
        u16le(c, 0);
        u16le(c, 0);
        u32le(c, 0);
        u32le(c, off);
        c.extend_from_slice(nb);
        self.count += 1;
    }

    pub fn finish(mut self) -> Vec<u8> {
        let cd_off = self.buf.len() as u32;
        let cd_len = self.central.len() as u32;
        let mut out = std::mem::take(&mut self.buf);
        out.extend_from_slice(&self.central);
        u32le(&mut out, 0x0605_4b50);
        u16le(&mut out, 0);
        u16le(&mut out, 0);
        u16le(&mut out, self.count);
        u16le(&mut out, self.count);
        u32le(&mut out, cd_len);
        u32le(&mut out, cd_off);
        u16le(&mut out, 0);
        out
    }
}

#[derive(serde::Serialize)]
pub struct Entry {
    pub name: String,
    pub offset: usize,
    pub len: usize,
    pub method: u16,
}

fn rd16(d: &[u8], i: usize) -> Option<u16> {
    d.get(i..i + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}
fn rd32(d: &[u8], i: usize) -> Option<u32> {
    d.get(i..i + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Читает оглавление ZIP. Для записей без сжатия (method 0) offset/len указывают на данные.
pub fn list(d: &[u8]) -> Option<Vec<Entry>> {
    if d.len() < 22 {
        return None;
    }
    let mut e = d.len() - 22;
    loop {
        if rd32(d, e)? == 0x0605_4b50 {
            break;
        }
        if e == 0 || d.len() - e > 70000 {
            return None;
        }
        e -= 1;
    }
    let n = rd16(d, e + 10)? as usize;
    let mut p = rd32(d, e + 16)? as usize;
    let mut out = Vec::new();
    for _ in 0..n {
        if rd32(d, p)? != 0x0201_4b50 {
            return None;
        }
        let method = rd16(d, p + 10)?;
        let csize = rd32(d, p + 20)? as usize;
        let nlen = rd16(d, p + 28)? as usize;
        let xlen = rd16(d, p + 30)? as usize;
        let clen = rd16(d, p + 32)? as usize;
        let loff = rd32(d, p + 42)? as usize;
        let name = String::from_utf8_lossy(d.get(p + 46..p + 46 + nlen)?).to_string();
        let lnl = rd16(d, loff + 26)? as usize;
        let lxl = rd16(d, loff + 28)? as usize;
        let data_off = loff + 30 + lnl + lxl;
        if data_off + csize > d.len() {
            return None;
        }
        out.push(Entry { name, offset: data_off, len: csize, method });
        p += 46 + nlen + xlen + clen;
    }
    Some(out)
}
