// Local Finder customization only. Never run against a release/signing artifact.
import AppKit
import ImageIO
import UniformTypeIdentifiers

let args = CommandLine.arguments
func fail(_ message: String) -> Never {
    fputs(message + "\n", stderr)
    exit(1)
}
guard args.count >= 3, ["apply", "restore", "inspect"].contains(args[1]) else {
    fail("Usage: macos-file-icon apply APP PNG | restore APP | inspect APP OUTPUT.png")
}
let bundle = URL(fileURLWithPath: args[2]).standardizedFileURL.resolvingSymlinksInPath()
guard bundle.pathExtension == "app",
      let info = NSDictionary(contentsOf: bundle.appendingPathComponent("Contents/Info.plist")),
      info["CFBundleIdentifier"] as? String == "cc.anjing.creatorx" else {
    fail("Target must be an installed CreatorX.app")
}
if args[1] != "inspect" {
    let parent = bundle.deletingLastPathComponent().path
    let userApps = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Applications").path
    guard parent == userApps || parent == "/Applications" else {
        fail("Only customize an installed app in ~/Applications or /Applications; never change build/release artifacts")
    }
}
if args[1] == "apply" {
    guard args.count == 4, let image = NSImage(contentsOfFile: args[3]) else { fail("Missing transparent source PNG") }
    guard NSWorkspace.shared.setIcon(image, forFile: bundle.path, options: []) else { fail("Finder did not accept the icon") }
    print("Applied local CreatorX file icon; Finder metadata is not part of signed release artifacts.")
} else if args[1] == "restore" {
    guard NSWorkspace.shared.setIcon(nil, forFile: bundle.path, options: []) else { fail("Unable to clear the local custom icon") }
    print("Cleared local custom icon. Restore the clean application backup for strict code-signature verification.")
} else {
    guard args.count == 4 else { fail("Missing output PNG") }
    let image = NSWorkspace.shared.icon(forFile: bundle.path)
    var bounds = CGRect(x: 0, y: 0, width: 512, height: 512)
    guard let cg = image.cgImage(forProposedRect: &bounds, context: nil, hints: nil),
          let output = CGImageDestinationCreateWithURL(URL(fileURLWithPath: args[3]) as CFURL, UTType.png.identifier as CFString, 1, nil) else { fail("Cannot read system-resolved icon") }
    CGImageDestinationAddImage(output, cg, nil)
    guard CGImageDestinationFinalize(output) else { fail("Cannot export system-resolved icon") }
    print("Exported icon resolved by NSWorkspace, including system decoration if any.")
}
