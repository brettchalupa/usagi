# Advanced Data

The `data` directory in Usagi games is very powerful! You can do all sorts of
things with it, like store your games levels, localization data, and more. Here
are a few examples:

- [Loading a level from CSV](https://codeberg.org/brettchalupa/usagi/src/branch/main/examples)
- [Loading a level from JSON](https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/level_from_json)
- [Loading localization data from JSON](https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/localization)

`usagi.read_text` and `usagi.read_json` allow you access the files in the `data`
folder, and they support live reload as well.

One nice thing about using text or JSON files in `data` is that you can read and
parse that data with other programming languages. For example, I put my game's
build version in `data/metadata.json`:

```json
{
  "build": "cara26.2"
}
```

In my Usagi game, I can display it for the player:

```lua
Metadata = usagi.read_json("metadata.json")

-- in _draw
gfx.text(Metadata.build, 10, 10, gfx.COLOR_BLACK)
```

And I can make use of it in my `push.rb` Ruby script that I use to deploy my
game to itch.io:

```ruby
#!/usr/bin/env ruby

# Script to deploy to itch

require "json"
DRY_RUN = ARGV.include?('--dry-run')

VER = JSON.parse(File.read("data/metadata.json"))["build"]

def butler_push(build)
  butler_cmd = "butler push --userversion=#{VER} export/neogear-summer-caravan-26-#{build}.zip brettchalupa/neogear-summer-caravan-26:#{build}"
  if DRY_RUN
    puts "[DRY RUN] pushing to itch: #{butler_cmd}"
  else
    system(butler_cmd)
  end
end

puts `usagi export`
butler_push('linux')
butler_push('macos')
butler_push('windows')
butler_push('web')
```

The `VER = JSON.parse(File.read("data/metadata.json"))["build"]` line reads that
JSON file and gets the `"build"` key's value.

If you want to share data across your game and other tooling, using the `./data`
directory is the way to go!
