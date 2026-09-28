// seams <png>: counts unpainted seams inside a blue selection highlight: short (<8pt)
// bands of rows that are almost free of highlight colour (<8%) between rows that are
// mostly highlighted, looked for in 120px-wide slices so partial seams count too.
// White glyphs never empty a whole band this way, so text does not trigger it.
import AppKit

let rep = NSBitmapImageRep(data: try! Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1])))!
let H = rep.pixelsHigh
func isBlue(_ c: NSColor) -> Bool { c.blueComponent > 0.8 && c.redComponent < 0.6 }
var found = Set<String>()
for x0 in stride(from: 40, to: rep.pixelsWide - 80, by: 120) {
  var frac = [Double](repeating: 0, count: H)
  for y in 0..<H {
    var n = 0, blue = 0
    for x in stride(from: x0, to: x0 + 120, by: 3) { n += 1; if isBlue(rep.colorAt(x: x, y: y)!.usingColorSpace(.deviceRGB)!) { blue += 1 } }
    frac[y] = Double(blue) / Double(n)
  }
  var y = 60
  while y < H - 60 {
    if frac[y] < 0.08 {
      let s = y
      while y < H && frac[y] < 0.08 { y += 1 }
      if y - s < 16 && frac[max(0, s - 3)] > 0.6 && frac[min(H - 1, y + 2)] > 0.6 { found.insert("\(s)-\(y - 1)") }
    } else { y += 1 }
  }
}
print(found.count)
