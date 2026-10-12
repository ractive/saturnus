# Template for the cask in ractive/homebrew-tap (Casks/saturnus-tx.rb).
# desktop.yml's cask job fills in @VERSION@ and @SHA256@ for a signed and
# notarised release and pushes the result; edit this file, not the tap's.
cask "saturnus-tx" do
  version "@VERSION@"
  sha256 "@SHA256@"

  url "https://github.com/ractive/saturnus/releases/download/v#{version}/saturnus-tx_#{version}_aarch64.dmg",
      verified: "github.com/ractive/saturnus/"
  name "saturnus tx"
  desc "Move files between HP 48 and 49 calculators on a serial cable and this computer"
  homepage "https://ractive.ch/saturnus/transfer/"

  livecheck do
    url :url
    strategy :github_latest
  end

  # Apple silicon only, as the saturnus-app cask: the app's minimum is
  # macOS 11, below Homebrew's own, so brew style wants the bare form.
  depends_on arch: :arm64
  depends_on :macos

  app "saturnus tx.app"

  zap trash: [
    "~/Library/Application Support/ch.ractive.saturnus-tx",
    "~/Library/Caches/ch.ractive.saturnus-tx",
    "~/Library/Preferences/ch.ractive.saturnus-tx.plist",
    "~/Library/Saved Application State/ch.ractive.saturnus-tx.savedState",
    "~/Library/WebKit/ch.ractive.saturnus-tx",
  ]
end
