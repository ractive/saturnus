# Template for the cask in ractive/homebrew-tap (Casks/saturnus-app.rb).
# desktop.yml's cask job fills in @VERSION@ and @SHA256@ for a signed and
# notarised release and pushes the result; edit this file, not the tap's.
cask "saturnus-app" do
  version "@VERSION@"
  sha256 "@SHA256@"

  url "https://github.com/ractive/saturnus/releases/download/v#{version}/saturnus_#{version}_aarch64.dmg",
      verified: "github.com/ractive/saturnus/"
  name "saturnus"
  desc "Emulator of the HP 48SX, 48GX, 49G, 38G, 39G, 40G and 42S calculators"
  homepage "https://ractive.ch/saturnus/"

  livecheck do
    url :url
    strategy :github_latest
  end

  # Apple silicon only. The app's minimum is macOS 11 (Tauri's 10.13,
  # raised to 11.0 for arm64), below Homebrew's own minimum, so brew style
  # wants the bare form rather than a redundant version.
  depends_on arch: :arm64
  depends_on :macos

  app "saturnus.app"

  zap trash: [
    "~/Library/Application Support/ch.ractive.saturnus",
    "~/Library/Caches/ch.ractive.saturnus",
    "~/Library/Preferences/ch.ractive.saturnus.plist",
    "~/Library/Saved Application State/ch.ractive.saturnus.savedState",
    "~/Library/WebKit/ch.ractive.saturnus",
  ]
end
