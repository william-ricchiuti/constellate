// Captioned App Store screenshots (2880x1800, 16:10) from the plain renders in ../screenshots:
// the wallpaper as a framed desktop under a headline and a subline, on the theme's own
// background colour. Uses only macOS frameworks.
// Usage: swift packaging/appstore/promo/make-promo.swift   -> packaging/appstore/promo/*.jpg
import AppKit

let here = URL(fileURLWithPath: CommandLine.arguments[0]).deletingLastPathComponent()
let src = here.appendingPathComponent("../screenshots").standardized
let (W, H): (CGFloat, CGFloat) = (2880, 1800)
let (SW, SH): (CGFloat, CGFloat) = (2304, 1440)
let frameTop: CGFloat = 300, radius: CGFloat = 34

struct Shot { let file, headline, subline: String; let dark: Bool }
let shots = [
  Shot(file: "1-night-sky", headline: "Your notes, as a night sky.",
       subline: "Every note is a star, every link a line. Right on your desktop.", dark: true),
  Shot(file: "2-aurora-lite", headline: "Calm by design.",
       subline: "It draws nothing when nothing changes, so it costs no battery.", dark: true),
  Shot(file: "3-ink-paper", headline: "Ink on warm paper.",
       subline: "Save a note and a little ink bleeds into the page.", dark: false),
  Shot(file: "4-pulse-burst", headline: "See what you just wrote.",
       subline: "Light runs out along the links from the note you saved.", dark: true),
  Shot(file: "5-default", headline: "Your notes, or any folder.",
       subline: "Markdown notes, wikilinks and tags, or every file in a folder.", dark: true),
]

func cornerColour(_ rep: NSBitmapImageRep) -> NSColor {
  var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, n: CGFloat = 0
  for x in stride(from: 0, to: 60, by: 4) { for y in stride(from: 0, to: 60, by: 4) {
    if let c = rep.colorAt(x: x, y: y)?.usingColorSpace(.sRGB) {
      r += c.redComponent; g += c.greenComponent; b += c.blueComponent; n += 1 } } }
  return NSColor(srgbRed: r / n, green: g / n, blue: b / n, alpha: 1)
}

for s in shots {
  guard let data = try? Data(contentsOf: src.appendingPathComponent("\(s.file).jpg")),
        let shotRep = NSBitmapImageRep(data: data), let shotCG = shotRep.cgImage else {
    fatalError("can't read \(s.file).jpg") }
  let bg = cornerColour(shotRep)
  let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(W), pixelsHigh: Int(H),
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
  NSGraphicsContext.saveGraphicsState()
  let ctx = NSGraphicsContext(bitmapImageRep: rep)!
  NSGraphicsContext.current = ctx
  let cg = ctx.cgContext
  bg.setFill(); NSRect(x: 0, y: 0, width: W, height: H).fill()

  // The wallpaper, framed, with a soft shadow (AppKit's origin is bottom-left).
  let frame = NSRect(x: (W - SW) / 2, y: H - frameTop - SH, width: SW, height: SH)
  let path = NSBezierPath(roundedRect: frame, xRadius: radius, yRadius: radius)
  cg.saveGState()
  cg.setShadow(offset: CGSize(width: 0, height: -18), blur: 60,
               color: NSColor.black.withAlphaComponent(s.dark ? 0.7 : 0.28).cgColor)
  bg.setFill(); path.fill()
  cg.restoreGState()
  cg.saveGState(); path.addClip(); cg.draw(shotCG, in: frame); cg.restoreGState()
  (s.dark ? NSColor.white.withAlphaComponent(0.10) : NSColor.black.withAlphaComponent(0.08)).setStroke()
  path.lineWidth = 2; path.stroke()

  // Headline and subline, centred above the frame.
  let ink = s.dark ? NSColor(white: 0.97, alpha: 1) : NSColor(srgbRed: 0.16, green: 0.15, blue: 0.13, alpha: 1)
  let centred = NSMutableParagraphStyle(); centred.alignment = .center
  let head: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: 104, weight: .semibold),
    .foregroundColor: ink, .paragraphStyle: centred, .kern: -1.0]
  let sub: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: 48, weight: .regular),
    .foregroundColor: ink.withAlphaComponent(0.72), .paragraphStyle: centred]
  NSAttributedString(string: s.headline, attributes: head).draw(in: NSRect(x: 0, y: H - 190, width: W, height: 130))
  NSAttributedString(string: s.subline, attributes: sub).draw(in: NSRect(x: 0, y: H - 262, width: W, height: 64))
  NSGraphicsContext.restoreGraphicsState()

  let out = here.appendingPathComponent("\(s.file).jpg")
  try! rep.representation(using: .jpeg, properties: [.compressionFactor: 0.9])!.write(to: out)
  print(out.path)
}
