class Usagi < Formula
  desc "Simple 2D Game Engine for Rapid Prototyping"
  homepage "https://usagiengine.com"
  version "1.3.2"
  license "Unlicense"

  # URLs and checksums are maintained by scripts/update_homebrew.rb — after a
  # release, run `ruby scripts/update_homebrew.rb` to refresh them from GitHub.
  if OS.mac?
    # Universal binary (Apple Silicon + Intel).
    url "https://github.com/brettchalupa/usagi/releases/download/v1.3.2/usagi-1.3.2-macos.tar.gz"
    sha256 "c927cf16f7518d16c109266eea4bab4d63d6f21d624d34801eb36e9c5501507b"
  elsif OS.linux? && Hardware::CPU.intel?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.3.2/usagi-1.3.2-linux-x86_64.tar.gz"
    sha256 "a2fc30b6e83f83fd3cd5661c168226f6eab2f3b446f33ccb7b6592bd15237250"
  elsif OS.linux? && Hardware::CPU.arm?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.3.2/usagi-1.3.2-linux-aarch64.tar.gz"
    sha256 "fdc376ee3cf4a2949693a84620a4e229502d1d9b562ea534af5e34d833d711bc"
  else
    odie "usagi: no prebuilt binary for this platform yet (supported: macOS, Linux x86_64/arm64)"
  end

  def install
    bin.install "usagi"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/usagi --version")
  end
end
