// drag <windowID> <x0> <y0> (<x> <y> <holdSec>)...
// A real mouse drag through CGEvent, in points relative to the window's top-left corner.
// While holding, it keeps sending small moves so the page gets mousemove events, like a
// hand would. The pointer is put back where it was afterwards.
import CoreGraphics
import Foundation

let a = CommandLine.arguments
let wid = CGWindowID(a[1])!
let info = ((CGWindowListCopyWindowInfo([.optionIncludingWindow], wid) as? [[String: Any]]) ?? []).first!
let b = info["kCGWindowBounds"] as! [String: Double]
func P(_ x: Double, _ y: Double) -> CGPoint { CGPoint(x: b["X"]! + x, y: b["Y"]! + y) }
let src = CGEventSource(stateID: .hidSystemState)
func post(_ t: CGEventType, _ p: CGPoint) {
  CGEvent(mouseEventSource: src, mouseType: t, mouseCursorPosition: p, mouseButton: .left)!.post(tap: .cghidEventTap)
}
let saved = CGEvent(source: nil)!.location
var p = P(Double(a[2])!, Double(a[3])!)
post(.mouseMoved, p); usleep(100_000)
post(.leftMouseDown, p); usleep(150_000)
var i = 4
while i + 2 < a.count {
  let q = P(Double(a[i])!, Double(a[i + 1])!), s = p
  for k in 1...20 {
    let t = Double(k) / 20
    p = CGPoint(x: s.x + (q.x - s.x) * t, y: s.y + (q.y - s.y) * t)
    post(.leftMouseDragged, p); usleep(15_000)
  }
  for k in 0..<Int(Double(a[i + 2])! * 10) {
    post(.leftMouseDragged, CGPoint(x: p.x + Double(k % 2), y: p.y)); usleep(100_000)
  }
  i += 3
}
post(.leftMouseUp, p); usleep(300_000)
post(.mouseMoved, saved)
