#!/usr/bin/env ruby
# Update Formula/usagi.rb with the URLs and sha256s from a GitHub release.
#
# Downloads the .sha256 sidecar files published alongside each release archive
# (no need to fetch the archives themselves) and rewrites the version, download
# URLs, and checksums in the formula in place.
#
# Only the platforms already present (uncommented) in the formula are touched.
# A platform whose asset is missing from the release is skipped with a warning,
# so this is safe to run before every platform exists. To start shipping a new
# platform (e.g. Linux arm64 in v1.1.0), uncomment its branch in the formula
# first, then run this script to fill in the URL and sha256.
#
# Usage:
#   scripts/update_homebrew.rb              # latest published GitHub release
#   scripts/update_homebrew.rb v1.1.0       # specific tag
#   scripts/update_homebrew.rb 1.1.0        # leading 'v' optional
#   scripts/update_homebrew.rb --dry-run    # print changes, don't write the file
#
# Requires `gh` (authenticated).

require "fileutils"
require "shellwords"

REPO = "brettchalupa/usagi"
FORMULA = File.expand_path("../Formula/usagi.rb", __dir__)

# Archive filename suffixes the formula installs from, matching
# `usagi-<version>-<suffix>` produced by .github/workflows/release.yml.
SUFFIXES = %w[
  macos-aarch64.tar.gz
  linux-x86_64.tar.gz
  linux-aarch64.tar.gz
].freeze

def sh!(cmd)
  puts "+ #{cmd}"
  system(cmd) or abort("[update_homebrew] failed: #{cmd}")
end

def capture!(cmd)
  out = `#{cmd}`
  abort("[update_homebrew] failed: #{cmd}") unless $?.success?
  out.strip
end

dry_run = !ARGV.delete("--dry-run").nil?

abort("[update_homebrew] required tool not on PATH: gh") unless
  system("command -v gh >/dev/null 2>&1")

tag = ARGV[0]
if tag.nil? || tag.empty?
  tag = capture!("gh release view --repo #{REPO} --json tagName --jq .tagName")
  puts "[update_homebrew] no tag passed, using latest published release: #{tag}"
end
tag = "v#{tag}" unless tag.start_with?("v")
version = tag.sub(/^v/, "")

work = File.expand_path("../tmp/homebrew/#{version}", __dir__)
FileUtils.mkdir_p(work)

# Grab every checksum sidecar in one shot; we filter per-platform below.
sh!("gh release download #{Shellwords.escape(tag)} --repo #{REPO} " \
    "--dir #{Shellwords.escape(work)} --clobber --pattern #{Shellwords.escape("*.sha256")}")

content = File.read(FORMULA)
original = content.dup

content.sub!(/^(\s*version )"[^"]*"/) { "#{$1}\"#{version}\"" } or
  abort("[update_homebrew] could not find a `version` line in #{FORMULA}")

updated = []
SUFFIXES.each do |suffix|
  sidecar = Dir.glob(File.join(work, "*-#{suffix}.sha256")).first
  unless sidecar
    puts "[update_homebrew] skip #{suffix}: no asset in #{tag}"
    next
  end
  sha = File.read(sidecar).split.first
  url = "https://github.com/#{REPO}/releases/download/#{tag}/usagi-#{version}-#{suffix}"

  # Match this platform's `url` line and the `sha256` line that follows it,
  # preserving indentation. Allows an empty placeholder sha to be filled in.
  block = /url "[^"]*#{Regexp.escape(suffix)}"\n(\s*)sha256 "[0-9a-f]*"/
  if content.sub!(block) { %(url "#{url}"\n#{$1}sha256 "#{sha}") }
    updated << suffix
  else
    puts "[update_homebrew] skip #{suffix}: no matching branch in formula " \
         "(uncomment it first if this platform now ships)"
  end
end

abort("[update_homebrew] nothing to update — no known platforms matched") if updated.empty?

if content == original
  puts "[update_homebrew] formula already up to date for #{tag}."
elsif dry_run
  puts "[update_homebrew] dry run — would update #{updated.join(", ")} to #{tag}:"
  puts content
else
  File.write(FORMULA, content)
  puts "[update_homebrew] updated #{File.basename(FORMULA)} to #{tag} " \
       "(#{updated.join(", ")}). Review the diff and commit."
end
