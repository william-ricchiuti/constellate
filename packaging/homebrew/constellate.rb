# Cask for William's tap, github.com/william-ricchiuti/homebrew-tap (Casks/constellate.rb).
# This copy is the template. `packaging/homebrew/bump-cask.sh <version> <dmg> <tap-dir>` writes
# it into the tap with the release's version and sha256; CI does the same and opens a PR.
#
# Homebrew 7+ refuses casks from untrusted taps: users run
# `brew trust --cask william-ricchiuti/tap/constellate` before installing (README).
#
# Up to 1.0.0-beta.1 this cask was `vault-wallpaper` (the app was "Vault Wallpaper"). The tap's
# cask_renames.json maps the old token to this one, so `brew update` moves those installs over.
cask "constellate" do
  version "1.0.0-beta.1"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"

  url "https://github.com/william-ricchiuti/constellate/releases/download/v#{version}/Constellate-#{version}.dmg"
  name "Constellate"
  desc "Obsidian vault graph as a live desktop wallpaper"
  homepage "https://github.com/william-ricchiuti/constellate"

  # Betas are GitHub pre-releases, which :github_latest skips; :github_releases sees them.
  livecheck do
    url :url
    strategy :github_releases
  end

  # Sparkle updates the app in place, so `brew upgrade` leaves it alone.
  auto_updates true
  # Same as ">= :ventura"; current `brew style` wants the bare symbol (Homebrew/OSDependsOn).
  depends_on macos: :ventura

  app "Constellate.app"

  # Start at Login registers the app as a login item; remove it with the app. Copies updated
  # from 1.0.0-beta.1 by Sparkle may still be called "Vault Wallpaper".
  uninstall quit:       "com.williamricchiuti.vaultwallpaper",
            login_item: ["Constellate", "Vault Wallpaper"]

  # Settings, layout cache and custom presets; WebKit data from the settings window; Sparkle's
  # state lives under Caches and Preferences. Exported PNGs in ~/Pictures are the user's.
  zap trash: [
    "~/Library/Application Support/Constellate",
    "~/Library/Application Support/Vault Wallpaper",
    "~/Library/Caches/com.williamricchiuti.vaultwallpaper",
    "~/Library/HTTPStorages/com.williamricchiuti.vaultwallpaper",
    "~/Library/Preferences/com.williamricchiuti.vaultwallpaper.plist",
    "~/Library/Saved Application State/com.williamricchiuti.vaultwallpaper.savedState",
    "~/Library/WebKit/com.williamricchiuti.vaultwallpaper",
  ]
end
