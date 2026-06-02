class Usagi < Formula
  desc "Simple 2D Game Engine for Rapid Prototyping"
  homepage "https://usagiengine.com"
  version "1.0.0"
  license "Unlicense"

  # URLs and checksums are maintained by scripts/update_homebrew.rb — after a
  # release, run `ruby scripts/update_homebrew.rb` to refresh them from GitHub.
  if OS.mac? && Hardware::CPU.arm?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.0.0/usagi-1.0.0-macos-aarch64.tar.gz"
    sha256 "f188e1c70a4bd6fa8b02510624b8f2c33999ba505d5f6426180b6f6c4bd22516"
  elsif OS.linux? && Hardware::CPU.intel?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.0.0/usagi-1.0.0-linux-x86_64.tar.gz"
    sha256 "3976fa2de170110e43fb5c2c951d8fef6130325265cf61f8b505a8a6e69dbbca"
  # NOTE TO SELF WHEN I MAKE v1.1.0 RELEASE: Linux arm64 ships starting
  # v1.1.0. After that release, uncomment the branch below, then run `ruby
  # scripts/update_homebrew.rb`. It fills in the URL and sha256 automatically.
  # Leave it commented until the linux-aarch64 asset actually exists, or `brew
  # install` on Linux arm64 404s.
  # elsif OS.linux? && Hardware::CPU.arm?
  #   url "https://github.com/brettchalupa/usagi/releases/download/v1.0.0/usagi-1.0.0-linux-aarch64.tar.gz"
  #   sha256 ""
  else
    odie "usagi: no prebuilt binary for this platform yet (supported: macOS arm64, Linux x86_64)"
  end

  def install
    bin.install "usagi"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/usagi --version")
  end
end
