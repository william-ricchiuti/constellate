//! Draws the app icon: a constellation (notes as stars, links as lines) shown as a Mac's
//! wallpaper. Pure CPU, no GPU needed.
//!
//!   cargo run --release -p vw-render --example app_icon -- packaging/icon/icon-1024.png [desktop|dock|display]
//!
//! `desktop`: the icon is the desktop itself, a menu bar over a night sky.
//! `dock` (the default, the shipped icon): the icon is the desktop itself, a night sky over a Dock.
//! `display`: a display on a light tile, with the night sky on its screen.
//! `packaging/build-app.sh` turns the PNG into `AppIcon.icns`.

const S: usize = 1024;

type V = (f32, f32);
type Rgb = [f32; 3];

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn hex(h: u32) -> Rgb {
    [
        ((h >> 16) & 255) as f32 / 255.0,
        ((h >> 8) & 255) as f32 / 255.0,
        (h & 255) as f32 / 255.0,
    ]
}

/// Composites `c` at coverage `a` over `dst` (premultiplied colour).
fn over(dst: &mut [f32; 4], c: Rgb, a: f32) {
    let a = a.clamp(0.0, 1.0);
    for i in 0..3 {
        dst[i] = dst[i] * (1.0 - a) + c[i] * a;
    }
    dst[3] += a * (1.0 - dst[3]);
}

/// Additive light (for glow): brightens without changing coverage.
fn add(dst: &mut [f32; 4], c: Rgb, a: f32) {
    for i in 0..3 {
        dst[i] += c[i] * a.max(0.0);
    }
}

/// Anti-aliased coverage of a shape from its signed distance (negative inside).
fn cov(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

fn len(p: V) -> f32 {
    (p.0 * p.0 + p.1 * p.1).sqrt()
}

fn seg_dist(p: V, a: V, b: V) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    len((p.0 - a.0 - dx * t, p.1 - a.1 - dy * t))
}

/// Signed distance to a rounded rectangle centred at `c` with half-size `h` and radius `r`.
fn rrect(p: V, c: V, h: V, r: f32) -> f32 {
    let q = ((p.0 - c.0).abs() - h.0 + r, (p.1 - c.1).abs() - h.1 + r);
    len((q.0.max(0.0), q.1.max(0.0))) + q.0.max(q.1).min(0.0) - r
}

/// Apple's icon body: a superellipse, 824 points across, centred on the 1024 canvas.
const C: f32 = 512.0;
const HALF: f32 = 412.0;
fn body(p: V, dy: f32) -> f32 {
    let n = 5.0f32;
    let (ux, uy) = ((p.0 - C).abs() / HALF, (p.1 - C - dy).abs() / HALF);
    (ux.powf(n) + uy.powf(n)).powf(1.0 / n) * HALF - HALF
}

/// The constellation in a 0..1 square: (x, y, radius as a share of the sky's width).
const STARS: [(f32, f32, f32); 7] = [
    (0.14, 0.64, 0.030),
    (0.31, 0.40, 0.026),
    (0.49, 0.53, 0.036),
    (0.66, 0.26, 0.0), // the bright star, drawn as a four-point sparkle
    (0.86, 0.36, 0.024),
    (0.58, 0.80, 0.028),
    (0.82, 0.72, 0.022),
];
const LINKS: [(usize, usize); 6] = [(0, 1), (1, 2), (2, 3), (3, 4), (2, 5), (5, 6)];
const BRIGHT: usize = 3;
/// Faint background stars, same 0..1 space: (x, y, radius in pixels at full size).
const DUST: [(f32, f32, f32); 12] = [
    (0.08, 0.20, 4.0),
    (0.24, 0.12, 3.0),
    (0.44, 0.22, 3.5),
    (0.92, 0.12, 4.0),
    (0.95, 0.58, 3.0),
    (0.70, 0.52, 3.0),
    (0.32, 0.72, 3.5),
    (0.06, 0.90, 3.0),
    (0.40, 0.94, 4.0),
    (0.74, 0.95, 3.0),
    (0.22, 0.52, 2.5),
    (0.96, 0.86, 3.5),
];

/// The night sky with its constellation, inside the box `x0..x1, y0..y1` (`k` scales
/// strokes and glows for a smaller sky).
struct Sky {
    x0: f32,
    y0: f32,
    w: f32,
    h: f32,
    k: f32,
}

impl Sky {
    fn at(&self, s: (f32, f32)) -> V {
        (self.x0 + s.0 * self.w, self.y0 + s.1 * self.h)
    }

    /// Paints the sky's colour and stars at `p` into `layer` (an opaque base must already be there).
    fn paint(&self, p: V, layer: &mut [f32; 4]) {
        let top = hex(0x2a1d63);
        let bottom = hex(0x0c0a1e);
        let violet = hex(0x7c5cff);
        let lilac = hex(0xd6ceff);
        let cyan = hex(0x7ee6ff);
        let pink = hex(0xff8fc8);
        let k = self.k;

        let t = ((p.1 - self.y0) / self.h).clamp(0.0, 1.0);
        let mut col = mix(top, bottom, t);
        let bloom_c = self.at((0.52, 0.45));
        let bloom = (-(len((p.0 - bloom_c.0, p.1 - bloom_c.1)) / (0.42 * self.w)).powi(2)).exp();
        col = mix(col, violet, bloom * 0.33);
        over(layer, col, 1.0);

        for &(x, y, r) in &DUST {
            let c = self.at((x, y));
            over(
                layer,
                lilac,
                0.55 * cov(len((p.0 - c.0, p.1 - c.1)) - r * k),
            );
        }
        for &(i, j) in &LINKS {
            let (a, b) = (STARS[i], STARS[j]);
            let d = seg_dist(p, self.at((a.0, a.1)), self.at((b.0, b.1)));
            add(layer, violet, 0.28 * (-(d / (26.0 * k)).powi(2)).exp());
            over(layer, mix(lilac, violet, 0.25), 0.85 * cov(d - 4.5 * k));
        }
        for (idx, &(x, y, r)) in STARS.iter().enumerate() {
            if idx == BRIGHT {
                continue;
            }
            let c = self.at((x, y));
            let r = r * self.w;
            let d = len((p.0 - c.0, p.1 - c.1));
            let tint = if idx == 5 { pink } else { lilac };
            add(
                layer,
                tint,
                0.45 * (-((d - r).max(0.0) / (34.0 * k)).powi(2)).exp(),
            );
            let core = mix(
                tint,
                [1.0; 3],
                (1.0 - d / r).clamp(0.0, 1.0).powf(1.2) * 0.8,
            );
            over(layer, core, cov(d - r));
        }
        // The bright star: a four-point sparkle with a cyan halo.
        let c = self.at((STARS[BRIGHT].0, STARS[BRIGHT].1));
        let (dx, dy) = ((p.0 - c.0).abs(), (p.1 - c.1).abs());
        let reach = 0.115 * self.w;
        add(
            layer,
            cyan,
            0.75 * (-(len((dx, dy)) / (0.075 * self.w)).powi(2)).exp(),
        );
        // Astroid-like four-point shape: (|x|^q + |y|^q)^(1/q) with q < 1.
        let q = 0.55f32;
        let f = ((dx / reach).powf(q) + (dy / reach).powf(q)).powf(1.0 / q);
        let star = ((1.0 - f) * reach / 2.0 + 0.5).clamp(0.0, 1.0);
        over(
            layer,
            mix(cyan, [1.0; 3], (1.0 - f).clamp(0.0, 1.0).powf(0.6)),
            star,
        );
    }
}

/// Small three-star glyph (the app's own menu-bar item), centred at `c`, size `s`.
fn glyph(p: V, c: V, s: f32) -> f32 {
    let pts = [
        (c.0 - s, c.1 + 0.55 * s),
        (c.0, c.1 - 0.6 * s),
        (c.0 + s, c.1 + 0.35 * s),
    ];
    let mut d = f32::MAX;
    for &q in &pts {
        d = d.min(len((p.0 - q.0, p.1 - q.1)) - 0.30 * s);
    }
    d = d.min(seg_dist(p, pts[0], pts[1]) - 0.09 * s);
    d.min(seg_dist(p, pts[1], pts[2]) - 0.09 * s)
}

fn desktop(p: V) -> [f32; 4] {
    let mut px = [0.0f32; 4];
    over(
        &mut px,
        [0.0; 3],
        0.22 * (1.0 - (body(p, 12.0) / 18.0).clamp(0.0, 1.0)),
    );
    let d = body(p, 0.0);
    let inside = cov(d);
    if inside <= 0.0 {
        return px;
    }
    let mut layer = [0.0f32; 4];
    let sky = Sky {
        x0: C - HALF,
        y0: 190.0,
        w: 2.0 * HALF,
        h: C + HALF - 190.0,
        k: 1.0,
    };
    sky.paint(p, &mut layer);

    // Menu bar: frosted light strip across the top, as on a Mac.
    let bar_bottom = 196.0;
    if p.1 < bar_bottom + 1.0 {
        let a = cov(p.1 - bar_bottom);
        over(&mut layer, hex(0xf2f3f9), 0.95 * a);
        let ink = hex(0x2b2f45);
        let y = 148.0;
        let mut m = len((p.0 - 214.0, p.1 - y)) - 17.0;
        m = m.min(rrect(p, (290.0, y), (44.0, 10.0), 10.0));
        m = m.min(rrect(p, (382.0, y), (32.0, 10.0), 10.0));
        m = m.min(rrect(p, (460.0, y), (28.0, 10.0), 10.0));
        m = m.min(rrect(p, (722.0, y), (30.0, 10.0), 10.0));
        over(&mut layer, ink, 0.75 * cov(m));
        over(&mut layer, hex(0x4b3fc4), cov(glyph(p, (796.0, y), 22.0)));
    }
    over(
        &mut layer,
        hex(0x000000),
        0.18 * cov((p.1 - bar_bottom - 1.0).abs() - 1.0),
    );

    // Thin top highlight on the rim.
    let t = ((p.1 - (C - HALF)) / (2.0 * HALF)).clamp(0.0, 1.0);
    over(
        &mut layer,
        [1.0; 3],
        (1.0 - (-d / 3.0).clamp(0.0, 1.0)) * (1.0 - t).powi(3) * 0.25,
    );
    over(
        &mut px,
        [layer[0].min(1.0), layer[1].min(1.0), layer[2].min(1.0)],
        inside,
    );
    px
}

fn dock(p: V) -> [f32; 4] {
    let mut px = [0.0f32; 4];
    over(
        &mut px,
        [0.0; 3],
        0.22 * (1.0 - (body(p, 12.0) / 18.0).clamp(0.0, 1.0)),
    );
    let d = body(p, 0.0);
    let inside = cov(d);
    if inside <= 0.0 {
        return px;
    }
    let mut layer = [0.0f32; 4];
    let sky = Sky {
        x0: C - HALF,
        y0: 120.0,
        w: 2.0 * HALF,
        h: 640.0,
        k: 1.0,
    };
    sky.paint(p, &mut layer);

    // Dock: a frosted shelf along the bottom with a row of app tiles, as on a Mac.
    let (dc, dh) = ((C, 842.0), (262.0, 50.0));
    let shelf = rrect(p, dc, dh, 30.0);
    over(&mut layer, hex(0xe9ecf8), 0.30 * cov(shelf));
    over(&mut layer, [1.0; 3], 0.35 * cov(shelf.abs() - 1.5));
    let tiles = [0x79b8ff, 0xff8fc8, 0x7ee6ff, 0xffd27a, 0xa9b2ff];
    let (size, gap) = (68.0f32, 22.0f32);
    let start = C - (5.0 * size + 4.0 * gap) / 2.0 + size / 2.0;
    for (i, &h) in tiles.iter().enumerate() {
        let c = (start + i as f32 * (size + gap), dc.1);
        let td = rrect(p, c, (size / 2.0, size / 2.0), 16.0);
        let shade = ((p.1 - (c.1 - size / 2.0)) / size).clamp(0.0, 1.0);
        over(
            &mut layer,
            mix(mix(hex(h), [1.0; 3], 0.25), hex(h), shade),
            cov(td),
        );
    }

    // Thin top highlight on the rim.
    let t = ((p.1 - (C - HALF)) / (2.0 * HALF)).clamp(0.0, 1.0);
    over(
        &mut layer,
        [1.0; 3],
        (1.0 - (-d / 3.0).clamp(0.0, 1.0)) * (1.0 - t).powi(3) * 0.25,
    );
    over(
        &mut px,
        [layer[0].min(1.0), layer[1].min(1.0), layer[2].min(1.0)],
        inside,
    );
    px
}

fn display(p: V) -> [f32; 4] {
    let mut px = [0.0f32; 4];
    over(
        &mut px,
        [0.0; 3],
        0.20 * (1.0 - (body(p, 12.0) / 18.0).clamp(0.0, 1.0)),
    );
    let d = body(p, 0.0);
    let inside = cov(d);
    if inside <= 0.0 {
        return px;
    }
    let mut layer = [0.0f32; 4];
    let t = ((p.1 - (C - HALF)) / (2.0 * HALF)).clamp(0.0, 1.0);
    over(&mut layer, mix(hex(0xf6f7fc), hex(0xd8dcea), t), 1.0);

    // Stand: neck, then foot, behind the display.
    let neck_t = ((p.1 - 690.0) / 120.0).clamp(0.0, 1.0);
    let neck_half = 62.0 + 16.0 * neck_t;
    let neck = ((p.0 - C).abs() - neck_half).max((p.1 - 750.0).abs() - 60.0);
    over(
        &mut layer,
        mix(hex(0xc9cedd), hex(0xb3b9cc), neck_t),
        cov(neck),
    );
    over(
        &mut layer,
        hex(0xaab1c5),
        cov(rrect(p, (C, 812.0), (150.0, 16.0), 16.0)),
    );

    // Display: soft shadow, dark bezel, then the screen with the sky.
    let (dc, dh) = ((C, 452.0), (372.0, 252.0));
    over(
        &mut layer,
        [0.0; 3],
        0.22 * (1.0 - (rrect(p, (dc.0, dc.1 + 14.0), dh, 44.0) / 26.0).clamp(0.0, 1.0)),
    );
    over(&mut layer, hex(0x23263a), cov(rrect(p, dc, dh, 44.0)));
    let sh = (dh.0 - 22.0, dh.1 - 22.0);
    let screen = rrect(p, dc, sh, 24.0);
    if screen < 1.0 {
        let mut s = [0.0f32; 4];
        let sky = Sky {
            x0: dc.0 - sh.0,
            y0: dc.1 - sh.1,
            w: 2.0 * sh.0,
            h: 2.0 * sh.1,
            k: 0.85,
        };
        sky.paint(p, &mut s);
        over(
            &mut layer,
            [s[0].min(1.0), s[1].min(1.0), s[2].min(1.0)],
            cov(screen),
        );
    }

    over(
        &mut layer,
        [1.0; 3],
        (1.0 - (-d / 3.0).clamp(0.0, 1.0)) * (1.0 - t).powi(3) * 0.5,
    );
    over(&mut px, [layer[0], layer[1], layer[2]], inside);
    px
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args
        .next()
        .unwrap_or_else(|| "packaging/icon/icon-1024.png".into());
    let style = args.next().unwrap_or_else(|| "dock".into());
    let draw: fn(V) -> [f32; 4] = match style.as_str() {
        "desktop" => desktop,
        "display" => display,
        "dock" => dock,
        other => panic!("unknown style {other} (desktop, dock or display)"),
    };

    let mut img = vec![0u8; S * S * 4];
    for y in 0..S {
        for x in 0..S {
            let px = draw((x as f32 + 0.5, y as f32 + 0.5));
            let i = (y * S + x) * 4;
            let a = px[3].clamp(0.0, 1.0);
            for ch in 0..3 {
                // Store straight (non-premultiplied) colour.
                let v = if a > 0.0 {
                    (px[ch] / a.max(1e-4)).min(1.0)
                } else {
                    0.0
                };
                img[i + ch] = (v * 255.0 + 0.5) as u8;
            }
            img[i + 3] = (a * 255.0 + 0.5) as u8;
        }
    }

    let file = std::fs::File::create(&out).expect("create output");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), S as u32, S as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .and_then(|mut w| w.write_image_data(&img))
        .expect("write png");
    println!("wrote {out}");
}
