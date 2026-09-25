#!/usr/bin/env ruby
#
# Upload the built ./_build to Bunny.net Storage Zone in parallel.
#
# Requires `bunny` CLI to be installed
# Supports optional `--dry-run` and `--jobs N` args

DRY_RUN = ARGV.include?("--dry-run").freeze
JOBS = (ARGV[ARGV.index("--jobs") + 1].to_i if ARGV.include?("--jobs")) || 8
DIR = ARGV.last || "_build".freeze

files = Dir.chdir(DIR) { Dir.glob("**/*").reject { |f| File.directory?(f) } }
queue = Queue.new
files.each { |f| queue << f }

workers = Array.new(JOBS) do
  Thread.new do
    while (f = queue.pop(true) rescue nil)
      command = "bunny storage files upload #{f} --to #{f}"
      if DRY_RUN
        puts "[DRYRUN] " + command
      else
        unless system(command)
          puts "FAILED: #{f}"
        end
      end
    end
  end
end

Dir.chdir(DIR) do
  workers.each(&:join)
end

system("bunny api -v POST /pullzone/6679857/purgeCache")
