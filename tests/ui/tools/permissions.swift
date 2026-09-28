// permissions: exits non-zero unless this process may record the screen and post input.
import ApplicationServices
import CoreGraphics

let screen = CGPreflightScreenCaptureAccess(), input = AXIsProcessTrusted() && CGPreflightPostEventAccess()
if !screen { print("missing: Screen Recording (System Settings > Privacy & Security) for the terminal running the tests") }
if !input { print("missing: Accessibility (System Settings > Privacy & Security) for the terminal running the tests") }
exit(screen && input ? 0 : 1)
