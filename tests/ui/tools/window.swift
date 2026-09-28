// window <pid>: prints "<windowID>|<title>" for the app's main (editor) window, if on screen.
import CoreGraphics
import Foundation

let pid = Int(CommandLine.arguments[1])!
let list = (CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]]) ?? []
for w in list where (w["kCGWindowOwnerPID"] as? Int) == pid && (w["kCGWindowLayer"] as? Int) == 0 {
  let b = w["kCGWindowBounds"] as! [String: Double]
  if b["Height"]! > 300 {
    print("\(w["kCGWindowNumber"]!)|\(w["kCGWindowName"] as? String ?? "")")
    break
  }
}
