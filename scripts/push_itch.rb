#!/usr/bin/env ruby
# Push usagi release archives to itch.io via butler.
#
# Downloads the linux/macos/windows archives from a GitHub release and uploads
# each to the matching itch.io channel under brettchalupa/usagi, tagged with
# the release version.
#
# Note on archive handling: butler auto-extracts zip archives and treats
# tar.gz as opaque blobs. We push the archives as-is on purpose — the macOS
# binary is unsigned, and pushing the extracted binary triggers Gatekeeper
# blocks; shipping the tarball makes users extract via Terminal, which avoids
# the quarantine path.
#
# Usage:
#   scripts/push_itch.rb              # latest published GitHub release
#   scripts/push_itch.rb v0.6.0       # specific tag
#   scripts/push_itch.rb 0.6.0        # leading 'v' optional
#   scripts/push_itch.rb --dry-run    # download archives but skip butler push
#
# Requires `gh` (authenticated) and `butler` (logged in: `butler login`).

require "fileutils"
require "shellwords"

TARGET = "brettchalupa/usagi"

# Archive filename suffix -> itch.io channel.
# Suffixes match `usagi-<version>-<suffix>` produced by .github/workflows/release.yml.
CHANNELS = {
  "linux-x86_64.tar.gz"  => "linux",
  "macos-aarch64.tar.gz" => "macos",
  "windows-x86_64.zip"   => "windows",
}

def sh!(cmd)
  puts "+ #{cmd}"
  system(cmd) or abort("[push_itch] failed: #{cmd}")
end

def capture!(cmd)
  out = `#{cmd}`
  abort("[push_itch] failed: #{cmd}") unless $?.success?
  out.strip
end

dry_run = !ARGV.delete("--dry-run").nil?

required = %w[gh]
required << "butler" unless dry_run
required.each do |bin|
  next if system("command -v #{bin} >/dev/null 2>&1")
  abort("[push_itch] required tool not on PATH: #{bin}")
end

tag = ARGV[0]
if tag.nil? || tag.empty?
  tag = capture!("gh release view --json tagName --jq .tagName")
  puts "[push_itch] no tag passed, using latest published release: #{tag}"
end
tag = "v#{tag}" unless tag.start_with?("v")
version = tag.sub(/^v/, "")

work = File.expand_path("../tmp/itch/#{version}", __dir__)
FileUtils.mkdir_p(work)

patterns = CHANNELS.keys.map { |s| "--pattern #{Shellwords.escape("*-#{s}")}" }.join(" ")
sh!("gh release download #{Shellwords.escape(tag)} --dir #{Shellwords.escape(work)} --clobber #{patterns}")

CHANNELS.each do |suffix, channel|
  archive = Dir.glob(File.join(work, "*-#{suffix}")).first
  abort("[push_itch] missing archive matching *-#{suffix} in #{work}") unless archive
  cmd = "butler push #{Shellwords.escape(archive)} #{TARGET}:#{channel} --userversion #{Shellwords.escape(version)}"
  if dry_run
    puts "[dry-run] #{cmd}"
  else
    sh!(cmd)
  end
end

if dry_run
  puts "[push_itch] dry run complete; archives in #{work}, nothing pushed."
else
  puts "[push_itch] done. Track build status with: butler status #{TARGET}"
end
