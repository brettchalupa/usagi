class Usagi < Formula
  desc "Simple 2D Game Engine for Rapid Prototyping"
  homepage "https://usagiengine.com"
  version "1.2.0"
  license "Unlicense"

  # URLs and checksums are maintained by scripts/update_homebrew.rb — after a
  # release, run `ruby scripts/update_homebrew.rb` to refresh them from GitHub.
  if OS.mac?
    # Universal binary (Apple Silicon + Intel).
    url "https://github.com/brettchalupa/usagi/releases/download/v1.2.0/usagi-1.2.0-macos.tar.gz"
    sha256 "2e8a388ee1c22adb9cfe2bab29668c76c88b1b4abfd68bf4aef2e7418631703d"
  elsif OS.linux? && Hardware::CPU.intel?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.2.0/usagi-1.2.0-linux-x86_64.tar.gz"
    sha256 "5e4d920bb926be7fc57578a49e115d0926169e969220fe34c254612460a40199"
  elsif OS.linux? && Hardware::CPU.arm?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.2.0/usagi-1.2.0-linux-aarch64.tar.gz"
    sha256 "3a0cdcd50a9683fc5622e50f473c7774f7d9ef81b850cebaf4a0bd7bcd2a9832"
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
