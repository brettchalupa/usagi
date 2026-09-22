class Usagi < Formula
  desc "Simple 2D Game Engine for Rapid Prototyping"
  homepage "https://usagiengine.com"
  version "1.3.3"
  license "Unlicense"

  # URLs and checksums are maintained by scripts/update_homebrew.rb — after a
  # release, run `ruby scripts/update_homebrew.rb` to refresh them from GitHub.
  if OS.mac?
    # Universal binary (Apple Silicon + Intel).
    url "https://github.com/brettchalupa/usagi/releases/download/v1.3.3/usagi-1.3.3-macos.tar.gz"
    sha256 "25b582f295ee6492dccd17aab6c2b9cebb1afc1119622a165734e1991d80fe4d"
  elsif OS.linux? && Hardware::CPU.intel?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.3.3/usagi-1.3.3-linux-x86_64.tar.gz"
    sha256 "26d5f57d4dc48da71324fbd93bd1e218bd2eaed939e1798dfeeb79089583f2b4"
  elsif OS.linux? && Hardware::CPU.arm?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.3.3/usagi-1.3.3-linux-aarch64.tar.gz"
    sha256 "ff6357b4ebebfdd2a87e3a0d6426dbf7633fc2dd745512cd4ddf6590c0c83472"
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
