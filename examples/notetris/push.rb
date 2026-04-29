#!/usr/bin/env ruby

def butler_push(build)
  puts `butler push export/notetris-#{build}.zip brettchalupa/notetris:#{build}`
end

puts `usagi export`
butler_push("linux")
butler_push("macos")
butler_push("windows")
butler_push("web")
