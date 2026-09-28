// highlight-diff <a.png> <b.png>: compares the selection highlight of two screenshots row
// by row (runs of highlight colour, merged across glyph gaps). Prints
// "<rows with highlight in a> <rows in b> <rows that differ> <max edge difference px>".
import AppKit

func load(_ p: String) -> NSBitmapImageRep { NSBitmapImageRep(data: try! Data(contentsOf: URL(fileURLWithPath: p)))! }
let A = load(CommandLine.arguments[1]), B = load(CommandLine.arguments[2])
func runs(_ r: NSBitmapImageRep, _ y: Int) -> [(Int, Int)] {
  var out: [(Int, Int)] = []
  var start = -1
  for x in 0..<r.pixelsWide {
    let c = r.colorAt(x: x, y: y)!.usingColorSpace(.deviceRGB)!
    let blue = c.blueComponent > 0.8 && c.redComponent < 0.55 && c.greenComponent > 0.4 && c.greenComponent < 0.75
    if blue && start < 0 { start = x }
    if !blue && start >= 0 { out.append((start, x - 1)); start = -1 }
  }
  if start >= 0 { out.append((start, r.pixelsWide - 1)) }
  var merged: [(Int, Int)] = []
  for r in out { if let l = merged.last, r.0 - l.1 < 30 { merged[merged.count - 1].1 = r.1 } else { merged.append(r) } }
  return merged
}
var rowsA = 0, rowsB = 0, diff = 0, maxD = 0
for y in 60..<(min(A.pixelsHigh, B.pixelsHigh) - 50) {
  let ra = runs(A, y), rb = runs(B, y)
  if !ra.isEmpty { rowsA += 1 }
  if !rb.isEmpty { rowsB += 1 }
  var d = 0
  if ra.count != rb.count { d = 9999 } else { for (p, q) in zip(ra, rb) { d = max(d, abs(p.0 - q.0), abs(p.1 - q.1)) } }
  if d > 0 { diff += 1; maxD = max(maxD, d) }
}
print(rowsA, rowsB, diff, maxD)
