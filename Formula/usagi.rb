class Usagi < Formula
  desc "Simple 2D Game Engine for Rapid Prototyping"
  homepage "https://usagiengine.com"
  version "1.3.0"
  license "Unlicense"

  # URLs and checksums are maintained by scripts/update_homebrew.rb — after a
  # release, run `ruby scripts/update_homebrew.rb` to refresh them from GitHub.
  if OS.mac?
    # Universal binary (Apple Silicon + Intel).
    url "https://github.com/brettchalupa/usagi/releases/download/v1.3.0/usagi-1.3.0-macos.tar.gz"
    sha256 "0e982ccb8357551913ad780722f52fd37d75b580685679f62b35642c60607ea4"
  elsif OS.linux? && Hardware::CPU.intel?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.3.0/usagi-1.3.0-linux-x86_64.tar.gz"
    sha256 "eb5845b7b6f364bcd8fd225a5938c3e17322f48c0048d8ac4027f8daabb8f2bd"
  elsif OS.linux? && Hardware::CPU.arm?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.3.0/usagi-1.3.0-linux-aarch64.tar.gz"
    sha256 "05ce38a9e4af64e743673e7d61974549d73313cddb2c25fbd0fb2dcf188b2d0f"
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
