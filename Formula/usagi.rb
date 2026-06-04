class Usagi < Formula
  desc "Simple 2D Game Engine for Rapid Prototyping"
  homepage "https://usagiengine.com"
  version "1.1.0"
  license "Unlicense"

  # URLs and checksums are maintained by scripts/update_homebrew.rb — after a
  # release, run `ruby scripts/update_homebrew.rb` to refresh them from GitHub.
  if OS.mac? && Hardware::CPU.arm?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.1.0/usagi-1.1.0-macos-aarch64.tar.gz"
    sha256 "ba3d2371aae64c8dc681fa2ad7883459f3583ffe30d53cb52f4622da2f4b1a80"
  elsif OS.linux? && Hardware::CPU.intel?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.1.0/usagi-1.1.0-linux-x86_64.tar.gz"
    sha256 "dbab2917f7b808778a18efef2229a0491b4be20fc573c8573e1256d69c5eb64a"
  elsif OS.linux? && Hardware::CPU.arm?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.1.0/usagi-1.1.0-linux-aarch64.tar.gz"
    sha256 "9f4d08e59ef37680aaea452fb362ab51cf5e45dacd1d6cc1fbaff7e9bb05155f"
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
