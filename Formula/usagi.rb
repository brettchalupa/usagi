class Usagi < Formula
  desc "Simple 2D Game Engine for Rapid Prototyping"
  homepage "https://usagiengine.com"
  version "1.0.0"

  if OS.mac? && Hardware::CPU.arm?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.0.0/usagi-1.0.0-macos-aarch64.tar.gz"
    sha256 "f188e1c70a4bd6fa8b02510624b8f2c33999ba505d5f6426180b6f6c4bd22516"
  elsif OS.linux?
    url "https://github.com/brettchalupa/usagi/releases/download/v1.0.0/usagi-1.0.0-linux-x86_64.tar.gz"
    sha256 "3976fa2de170110e43fb5c2c951d8fef6130325265cf61f8b505a8a6e69dbbca"
  end

  def install
    bin.install "usagi"
  end

  test do
    system "#{bin}/usagi", "--version"
  end
end
